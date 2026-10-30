from pathlib import Path
ROOT=Path('/code/aster/.wt-deepseek-platform-runtime')
POWER=ROOT/'apps/cloud/crates/power'
COPY=ROOT/'crates/power'

def edit(relative,old,new):
    a,b=POWER/relative,COPY/relative
    s=a.read_text();assert s==b.read_text();assert s.count(old)==1,(relative,old[:60])
    s=s.replace(old,new,1);a.write_text(s);b.write_text(s)

edit('src/config.rs','pub struct XolotlConfig {\n','pub struct XolotlConfig {\n    /// Maximum indexer score elements in one graph piece; unset preserves upstream.\n    #[serde(default, skip_serializing_if = "Option::is_none")]\n    pub indexer_tile_elements: Option<u64>,\n')
edit('src/config.rs','            dspark: None,','            indexer_tile_elements: None,\n            dspark: None,')
edit('src/config.rs','impl Default for XolotlConfig {','''impl XolotlConfig {
    pub(crate) fn validate_indexer_tile(&self) -> Result<()> {
        if self.indexer_tile_elements.is_some_and(|n| !(1 << 24..=1 << 28).contains(&n) || !n.is_power_of_two()) {
            return Err(PowerError::Config("xolotl.indexer_tile_elements must be a power of two from 2^24 through 2^28".into()));
        }
        Ok(())
    }
}
impl Default for XolotlConfig {''')
edit('src/config/behavior.rs','        if let Some(dspark) = &self.xolotl.dspark {','        self.xolotl.validate_indexer_tile()?;\n        if let Some(dspark) = &self.xolotl.dspark {')
edit('src/config/acl.rs','        .attribute("kv_fp4", AttributeSchema::optional(ValueSchema::bool()))','        .attribute("indexer_tile_elements", AttributeSchema::optional(ValueSchema::number()))\n        .attribute("kv_fp4", AttributeSchema::optional(ValueSchema::bool()))')
edit('src/backend/xolotl/plan.rs','    if let Some(dspark) = &xolotl.dspark {','''    xolotl.validate_indexer_tile()?;
    if let Some(tile) = xolotl.indexer_tile_elements {
        launch.env.push(("STRATA_INDEXER_TILE".into(), tile.to_string()));
    }
    if let Some(dspark) = &xolotl.dspark {''')
edit('src/backend/xolotl/managed.rs','"CUDA_VISIBLE_DEVICES"\n                    | "STRATA_SERVE_DSPARK"','"CUDA_VISIBLE_DEVICES"\n                    | "STRATA_INDEXER_TILE"\n                    | "STRATA_SERVE_DSPARK"')

p=POWER/'src/config/xolotl_dspark_tests.rs'
s=p.read_text()+'''
#[test]
fn indexer_tile_is_optional_canonical_and_bounded_through_acl() {
    assert!(PowerConfig::default().xolotl.indexer_tile_elements.is_none());
    for n in [1_u64<<24,1<<25,1<<26,1<<27,1<<28] {
        let config=super::super::acl::deserialize(&format!("xolotl {{ indexer_tile_elements = {n} }}")).unwrap();
        assert_eq!(config.xolotl.indexer_tile_elements,Some(n));
        config.xolotl.validate_indexer_tile().unwrap();
    }
    for n in [0_u64,(1<<24)-1,(1<<26)+1,1<<29] {
        let mut c=PowerConfig::default();c.xolotl.indexer_tile_elements=Some(n);
        assert!(c.xolotl.validate_indexer_tile().is_err());
        assert!(c.validate().is_err());
    }
    assert!(super::super::acl::deserialize("xolotl { indexer_tile_elements = \\"67108864\\" }").is_err());
}
'''
p.write_text(s);(COPY/'src/config/xolotl_dspark_tests.rs').write_text(s)

p=ROOT/'tools/model-runtime/deepseek-v41-xq/power_xolotl_bridge.py'
s=p.read_text()
old='    if not isinstance(env, dict) or set(env) not in ({"CUDA_VISIBLE_DEVICES"}, {"CUDA_VISIBLE_DEVICES"} | draft_keys):'
assert s.count(old)==1
s=s.replace(old,'    if not isinstance(env, dict) or "CUDA_VISIBLE_DEVICES" not in env or (set(env) - {"CUDA_VISIBLE_DEVICES", "STRATA_INDEXER_TILE"}) not in (set(), draft_keys):',1)
anchor='    if draft_keys <= set(env):\n'
assert s.count(anchor)==1
s=s.replace(anchor,'''    if "STRATA_INDEXER_TILE" in env:
        tile = text(env["STRATA_INDEXER_TILE"], "indexer tile")
        if not tile.isascii() or not tile.isdecimal() or str(int(tile)) != tile or not (1 << 24) <= int(tile) <= (1 << 28) or int(tile) & (int(tile) - 1):
            raise ValueError("Indexer tile exceeds canonical power-of-two bounds")
    if draft_keys <= set(env):
''',1)
s=s.replace('clear_inherited_dspark','clear_inherited_tuning')
s=s.replace('"STRATA_SERVE_DSPARK_EVICT", "STRATA_DSPARK"):', '"STRATA_SERVE_DSPARK_EVICT", "STRATA_DSPARK", "STRATA_INDEXER_TILE"):')
p.write_text(s)
p=p.parent/'test_power_xolotl_bridge.py';s=p.read_text().replace('clear_inherited_dspark','clear_inherited_tuning')
marker='class Boundary(unittest.TestCase):\n'
test='''class Boundary(unittest.TestCase):
    def test_indexer_tile_is_optional_and_does_not_open_arbitrary_environment(self):
        value=launch();value["engine_env"]["STRATA_INDEXER_TILE"]="67108864"
        self.assertEqual(decode(value),value)
        for bad in ["0","16777215","67108865","536870912","067108864"]:
            v=copy.deepcopy(value);v["engine_env"]["STRATA_INDEXER_TILE"]=bad
            with self.subTest(bad=bad),self.assertRaises(ValueError):decode(v)
        with patch.dict(os.environ,{"STRATA_INDEXER_TILE":"1"}):
            bridge.clear_inherited_tuning();self.assertNotIn("STRATA_INDEXER_TILE",os.environ)

'''
assert s.count(marker)==1;p.write_text(s.replace(marker,test,1))
print('Bounded indexer graph tiling exposed through ACL; inherited value removed')
