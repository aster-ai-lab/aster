use super::*;

fn maps() -> (Vec<i32>, Vec<i32>) {
    let mut gu: Vec<_> = (0..EXPERTS)
        .map(|i| [42, 143, 42, 11, 143][i % 5])
        .collect();
    let down = (0..EXPERTS).map(|i| [143, 143, 42][i % 3]).collect();
    gu[383] = 11;
    (gu, down)
}

fn string(value: &str, out: &mut Vec<u8>) {
    out.extend((value.len() as u64).to_le_bytes());
    out.extend(value.as_bytes());
}

fn catalog(gu: &[i32], down: &[i32], broken: Option<(&str, &str)>) -> XqGgufHeader {
    let (_, gu_counts) = slots(gu).unwrap();
    let (_, down_counts) = slots(down).unwrap();
    let mut tensors = Vec::new();
    let mut offset = 0_u64;
    for (role, counts) in [
        (XqExpertRole::Gate, gu_counts),
        (XqExpertRole::Up, gu_counts),
        (XqExpertRole::Down, down_counts),
    ] {
        for (kind, count) in TIERS.into_iter().zip(counts) {
            if count == 0 {
                continue;
            }
            let name = tensor_name(0, role, kind).unwrap();
            if broken == Some((name.as_str(), "missing")) {
                continue;
            }
            let (rows, columns) = role.dimensions();
            let mut shape = vec![columns, rows, count];
            let mut actual_kind = kind;
            if let Some((target, change)) = broken {
                if target == name {
                    match change {
                        "count" => shape[2] += 1,
                        "width" => shape[0] += 256,
                        "type" => actual_kind = XqTensorType::Q8_0,
                        _ => panic!("invalid fixture change"),
                    }
                }
            }
            tensors.push((name, shape.clone(), actual_kind, offset));
            offset = (offset + actual_kind.tensor_bytes(&shape).unwrap()).div_ceil(32) * 32;
        }
    }
    let mut header = b"GGUF".to_vec();
    header.extend(3_u32.to_le_bytes());
    header.extend((tensors.len() as u64).to_le_bytes());
    header.extend(1_u64.to_le_bytes());
    string("general.alignment", &mut header);
    header.extend(4_u32.to_le_bytes());
    header.extend(32_u32.to_le_bytes());
    for (name, shape, kind, offset) in tensors {
        string(&name, &mut header);
        header.extend((shape.len() as u32).to_le_bytes());
        for value in shape {
            header.extend(value.to_le_bytes());
        }
        header.extend(kind.gguf_id().to_le_bytes());
        header.extend(offset.to_le_bytes());
    }
    header.resize(header.len().div_ceil(32) * 32, 0);
    XqGgufHeader::parse(&header, header.len() as u64 + offset).unwrap()
}

#[test]
fn independent_gu_and_down_maps_use_rank_within_each_type() {
    let (gu, down) = maps();
    let header = catalog(&gu, &down, None);
    let layout = XqExpertLayout::from_maps(&header, 0, &gu, &down).unwrap();
    for expert in 0..EXPERTS {
        for (role, map) in [
            (XqExpertRole::Gate, &gu),
            (XqExpertRole::Up, &gu),
            (XqExpertRole::Down, &down),
        ] {
            let p = layout.projection(expert, role).unwrap();
            assert_eq!(p.kind.gguf_id(), map[expert] as u32);
            let slot = map[..expert]
                .iter()
                .filter(|value| **value == map[expert])
                .count();
            let (rows, columns) = role.dimensions();
            assert_eq!(
                (p.first_row, p.rows, p.columns),
                (slot as u64 * rows, rows, columns)
            );
        }
    }
    assert!(layout.projection(EXPERTS, XqExpertRole::Gate).is_err());
    assert_eq!(layout.layer(), 0);
}

#[test]
fn maps_and_roles_have_closed_types_counts_and_layer_bounds() {
    let (gu, down) = maps();
    let header = catalog(&gu, &down, None);
    for value in [-1, 0, 1, 8, 26, 144] {
        let mut invalid = gu.clone();
        invalid[0] = value;
        assert!(XqExpertLayout::from_maps(&header, 0, &invalid, &down).is_err());
    }
    assert!(slots(&gu[..383]).is_err());
    let mut long = gu.clone();
    long.push(42);
    assert!(slots(&long).is_err());
    assert!(XqExpertLayout::from_maps(&header, LAYERS, &gu, &down).is_err());
    for value in ["gate", "up", "down"] {
        assert_eq!(XqExpertRole::parse(value).unwrap().as_str(), value);
    }
    for value in ["", "GU", "d", "weight"] {
        assert!(XqExpertRole::parse(value).is_err());
    }
}

#[test]
fn every_partition_must_match_all_three_stack_shapes_and_types() {
    let (gu, down) = maps();
    for role in [XqExpertRole::Gate, XqExpertRole::Up, XqExpertRole::Down] {
        for kind in TIERS {
            if role == XqExpertRole::Down && kind == XqTensorType::Q3K {
                continue;
            }
            let name = tensor_name(0, role, kind).unwrap();
            for change in ["missing", "count", "width", "type"] {
                let header = catalog(&gu, &down, Some((&name, change)));
                assert!(
                    XqExpertLayout::from_maps(&header, 0, &gu, &down).is_err(),
                    "{name} {change}"
                );
            }
        }
    }
}

#[test]
fn projected_rows_cannot_spill_into_the_next_expert() {
    let projection = XqExpertProjection {
        tensor_name: "unused".into(),
        kind: XqTensorType::Ptq1_0,
        first_row: 0,
        rows: INTERMEDIATE,
        columns: HIDDEN,
    };
    // Overflow must fail before any physical range is requested.
    for (first, rows) in [
        (INTERMEDIATE, 1),
        (INTERMEDIATE - 1, 2),
        (0, 0),
        (u64::MAX, 2),
    ] {
        assert!(projection.row_offset(first, rows).is_err());
    }
    assert_eq!(
        projection.row_offset(INTERMEDIATE - 1, 1).unwrap(),
        INTERMEDIATE - 1
    );
}
