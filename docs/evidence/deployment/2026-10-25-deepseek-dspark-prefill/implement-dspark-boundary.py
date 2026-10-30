from pathlib import Path

ROOT = Path('/code/aster/.wt-deepseek-platform-runtime')
POWER = ROOT / 'apps/cloud/crates/power'
COPY = ROOT / 'crates/power'


def edit(relative, old, new):
    a, b = POWER / relative, COPY / relative
    source = a.read_text()
    assert source == b.read_text(), relative
    assert source.count(old) == 1, (relative, old[:100])
    source = source.replace(old, new, 1)
    a.write_text(source)
    b.write_text(source)


def add(relative, source):
    for base in [POWER, COPY]:
        target = base / relative
        assert not target.exists(), target
        target.write_text(source)


add('src/config/xolotl_dspark.rs', '''use super::*;

/// Explicit ownership of the checkpoint's speculative draft. No arbitrary engine environment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XolotlDSparkConfig {
    pub sidecar: PathBuf,
    pub sidecar_sha256: String,
    pub depth: u32,
    pub confidence_bp: u32,
    pub evict_prompt_tokens: u32,
}
impl XolotlDSparkConfig {
    pub fn validate(&self) -> Result<()> {
        let path = self.sidecar.to_str().ok_or_else(|| PowerError::Config("xolotl.dspark sidecar path must be UTF-8".into()))?;
        if !self.sidecar.is_absolute() || path.len() > 4096 || path.contains(['\\0', '\\r', '\\n'])
            || self.sidecar_sha256.len() != 64
            || !self.sidecar_sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || !(1..=5).contains(&self.depth) || self.confidence_bp > 10000
            || self.evict_prompt_tokens > 1048576 || self.evict_prompt_tokens % 128 != 0 {
            return Err(PowerError::Config("xolotl.dspark exceeds its admitted path, digest or speculative bounds".into()));
        }
        Ok(())
    }
    pub(crate) fn append_env(&self, env: &mut Vec<(String, String)>) -> Result<()> {
        self.validate()?;
        let path = self.sidecar.to_str().ok_or_else(|| PowerError::Config("xolotl.dspark sidecar path must be UTF-8".into()))?;
        for (key, value) in [
            ("STRATA_SERVE_DSPARK", path.to_owned()),
            ("STRATA_DSPARK_DEPTH", self.depth.to_string()),
            ("STRATA_DSPARK_CONF", format!("{}.{:04}", self.confidence_bp / 10000, self.confidence_bp % 10000)),
            ("STRATA_SERVE_DSPARK_EVICT", self.evict_prompt_tokens.to_string()),
        ] { env.push((key.into(), value)); }
        Ok(())
    }
}
#[cfg(test)]
#[path = "xolotl_dspark_tests.rs"]
mod tests;
''')
edit('src/config.rs', 'use crate::speculative::SpeculativeStrategy;\n', 'use crate::speculative::SpeculativeStrategy;\n\nmod xolotl_dspark;\npub use xolotl_dspark::XolotlDSparkConfig;\n')
edit('src/config.rs', 'pub struct XolotlConfig {\n', 'pub struct XolotlConfig {\n    #[serde(default, skip_serializing_if = "Option::is_none")]\n    pub dspark: Option<XolotlDSparkConfig>,\n')
edit('src/config.rs', '            kv_fp4: false,\n', '            dspark: None,\n            kv_fp4: false,\n')
edit('src/config/acl.rs', 'fn xolotl_schema() -> Schema {\n', '''fn xolotl_schema() -> Schema {
    let dspark = ObjectSchema::new()
        .field("sidecar", AttributeSchema::required(ValueSchema::string()))
        .field("sidecar_sha256", AttributeSchema::required(ValueSchema::string()))
        .field("depth", AttributeSchema::required(ValueSchema::number()))
        .field("confidence_bp", AttributeSchema::required(ValueSchema::number()))
        .field("evict_prompt_tokens", AttributeSchema::required(ValueSchema::number()));
''')
edit('src/config/acl.rs', '    Schema::new()\n        .attribute(\n            "tuning",', '    Schema::new()\n        .attribute("dspark", AttributeSchema::optional(ValueSchema::object(dspark)))\n        .attribute(\n            "tuning",')
edit('src/backend/xolotl/plan.rs', '    if xolotl.kv_fp4 {\n', '''    if let Some(dspark) = &xolotl.dspark {
        if xolotl.gpus.len() != 1 || xolotl.layer_split != RTX_4090_LAYER_SPLIT
            || xolotl.cuda_architecture != RTX_4090_COMPUTE_CAPABILITY
            || xolotl.managed.is_none() || xolotl.tuning.is_none() || !xolotl.kv_fp4 {
            return Err(PowerError::Config("xolotl.dspark requires the managed single-4090 FP4-KV plan with explicit tuning".into()));
        }
        dspark.append_env(&mut launch.env)?;
    }
    if xolotl.kv_fp4 {
''')
edit('src/backend/xolotl/prepare.rs', '    let result = tokio::task::spawn_blocking(move || assembly.prepare())\n', '''    let dspark = config.xolotl.dspark.clone();
    let result = tokio::task::spawn_blocking(move || {
        if let Some(dspark) = dspark {
            dspark.validate()?;
            let size = regular_file(&dspark.sidecar)?.metadata()?.len();
            if !(4..=16 * 1024 * 1024 * 1024).contains(&size) {
                return Err(refused("DSpark sidecar exceeds its 16 GiB bound"));
            }
            // Reuse the regular-file, cancellation, exact digest and GGUF-magic proof.
            // This value is never registered as a second served model.
            Assembly {
                name: "DSpark draft".into(), path: dspark.sidecar,
                expected: dspark.sidecar_sha256, size, parts: Vec::new(),
                deadline: assembly.deadline, cancel: assembly.cancel.clone(),
            }.verify()?;
        }
        assembly.prepare()
    })
''')
edit('src/backend/xolotl/managed.rs', '    context_tokens: u32,\n}', '    context_tokens: u32,\n    #[serde(default)]\n    dspark_active: bool,\n}')
edit('src/backend/xolotl/managed.rs', '        let launch = Launch {\n', '''        let mut engine_env = BTreeMap::new();
        for (key, value) in &plan.env {
            if !matches!(key.as_str(), "CUDA_VISIBLE_DEVICES" | "STRATA_SERVE_DSPARK" | "STRATA_DSPARK_DEPTH" | "STRATA_DSPARK_CONF" | "STRATA_SERVE_DSPARK_EVICT")
                || engine_env.insert(key.as_str(), value.as_str()).is_some() {
                return Err(refused("managed Xolotl environment exceeds its typed launch authority"));
            }
        }
        engine_env.insert("CUDA_VISIBLE_DEVICES", visible_devices.as_str());
        let launch = Launch {
''')
edit('src/backend/xolotl/managed.rs', '            engine_env: BTreeMap::from([("CUDA_VISIBLE_DEVICES", visible_devices.as_str())]),', '            engine_env,')
edit('src/backend/xolotl/managed.rs', '                validate_context_proof(plan, proof.context_tokens)?;\n', '''                validate_context_proof(plan, proof.context_tokens)?;
                let expected_dspark = plan.env.iter().any(|(key, _)| key == "STRATA_SERVE_DSPARK");
                if proof.dspark_active != expected_dspark {
                    return Err(refused("managed Xolotl did not prove its admitted DSpark state"));
                }
''')

add('src/config/xolotl_dspark_tests.rs', '''use super::*;

fn valid() -> XolotlDSparkConfig {
    XolotlDSparkConfig { sidecar: "/models/draft.gguf".into(), sidecar_sha256: "a".repeat(64), depth: 4, confidence_bp: 6000, evict_prompt_tokens: 4096 }
}
#[test]
fn typed_draft_emits_only_the_closed_canonical_environment() {
    let mut env = vec![]; valid().append_env(&mut env).unwrap();
    assert_eq!(env, vec![("STRATA_SERVE_DSPARK".into(), "/models/draft.gguf".into()), ("STRATA_DSPARK_DEPTH".into(), "4".into()), ("STRATA_DSPARK_CONF".into(), "0.6000".into()), ("STRATA_SERVE_DSPARK_EVICT".into(), "4096".into())]);
}
#[test]
fn rejects_invalid_paths_digests_and_bounds() {
    let mut cases = vec![];
    let mut c=valid(); c.sidecar="relative.gguf".into(); cases.push(c);
    let mut c=valid(); c.sidecar="/model/bad\\npath".into(); cases.push(c);
    let mut c=valid(); c.sidecar_sha256="A".repeat(64); cases.push(c);
    let mut c=valid(); c.sidecar_sha256="a".repeat(63); cases.push(c);
    for n in [0,6] { let mut c=valid(); c.depth=n; cases.push(c); }
    let mut c=valid(); c.confidence_bp=10001; cases.push(c);
    for n in [1,1048704] { let mut c=valid(); c.evict_prompt_tokens=n; cases.push(c); }
    assert!(cases.iter().all(|c| c.validate().is_err()));
}
#[test]
fn acl_roundtrip_and_unknown_or_incomplete_draft_are_closed() {
    let source = "xolotl { dspark = { sidecar = \\\"/models/draft.gguf\\\", sidecar_sha256 = \\\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\\\", depth = 4, confidence_bp = 6000, evict_prompt_tokens = 4096 } }";
    let c = super::super::acl::deserialize(source).unwrap();
    assert_eq!(c.xolotl.dspark.as_ref(), Some(&valid()));
    let encoded = super::super::acl::serialize(&c).unwrap();
    assert_eq!(super::super::acl::deserialize(&encoded).unwrap().xolotl.dspark, Some(valid()));
    assert!(super::super::acl::deserialize("xolotl { dspark = { depth = 4 } }").is_err());
    assert!(super::super::acl::deserialize(&source.replace("depth = 4", "depth = 4, arbitrary_env = true")).is_err());
}
''')

bridge = ROOT / 'tools/model-runtime/deepseek-v41-xq/power_xolotl_bridge.py'
source = bridge.read_text()
old = '    if not isinstance(env, dict) or set(env) != {"CUDA_VISIBLE_DEVICES"}:\n'
assert source.count(old) == 1
source = source.replace(old, '''    draft_keys = {"STRATA_SERVE_DSPARK", "STRATA_DSPARK_DEPTH", "STRATA_DSPARK_CONF", "STRATA_SERVE_DSPARK_EVICT"}
    if not isinstance(env, dict) or set(env) not in ({"CUDA_VISIBLE_DEVICES"}, {"CUDA_VISIBLE_DEVICES"} | draft_keys):
''', 1)
old = '    indices = text(env["CUDA_VISIBLE_DEVICES"], "CUDA placement", 128).split(",")\n'
assert source.count(old) == 1
source = source.replace(old, '''    if draft_keys <= set(env):
        if split or not Path(text(env["STRATA_SERVE_DSPARK"], "DSpark sidecar", 4096)).is_absolute():
            raise ValueError("DSpark requires an absolute sidecar on the admitted single GPU")
        for key, lower, upper, step in [("STRATA_DSPARK_DEPTH", 1, 5, 1), ("STRATA_SERVE_DSPARK_EVICT", 0, 1048576, 128)]:
            value = text(env[key], key)
            if not value.isascii() or not value.isdecimal() or str(int(value)) != value or not lower <= int(value) <= upper or int(value) % step:
                raise ValueError("DSpark integer exceeds canonical bounds")
        confidence = text(env["STRATA_DSPARK_CONF"], "DSpark confidence")
        if len(confidence) != 6 or confidence[:2] not in ("0.", "1.") or not confidence[2:].isascii() or not confidence[2:].isdecimal() or (confidence[0] == "1" and confidence[2:] != "0000"):
            raise ValueError("DSpark confidence exceeds canonical basis points")
    indices = text(env["CUDA_VISIBLE_DEVICES"], "CUDA placement", 128).split(",")
''', 1)
marker = '\ndef run_owned(value, protocol):\n'
assert source.count(marker) == 1
source = source.replace(marker, '''
def dspark_proof(log, start, env):
    if "STRATA_SERVE_DSPARK" not in env:
        return False
    if type(start) is not int or start < 0:
        raise ValueError("DSpark omitted its current process log boundary")
    fd = os.open(log, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK)
    with os.fdopen(fd, "rb") as stream:
        info = os.fstat(stream.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_size < start or info.st_size - start > 8 * 1024 * 1024:
            raise ValueError("DSpark startup proof exceeds its regular-file bound")
        stream.seek(start)
        lines = stream.read(8 * 1024 * 1024 + 1).decode("utf-8", errors="strict").splitlines()
    prefix = "DSpark: " + env["STRATA_SERVE_DSPARK"] + ", "
    if not any(line.startswith(prefix) and " GB of weights on the card (" in line for line in lines):
        raise ValueError("DSpark did not load on the current owned engine")
    return True

def run_owned(value, protocol):
''', 1)
old = '                                         "context_tokens": context_proof(self.max_context, args)})'
assert source.count(old) == 1
source = source.replace(old, '''                                         "context_tokens": context_proof(self.max_context, args),
                                         "dspark_active": dspark_proof(self.log_path, getattr(self, "log_start", None), value["engine_env"])})''', 1)
bridge.write_text(source)
print('Typed DSpark config, measured sidecar, closed IPC environment and real startup proof implemented')
