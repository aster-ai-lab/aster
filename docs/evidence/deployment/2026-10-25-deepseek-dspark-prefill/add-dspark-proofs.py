from pathlib import Path
ROOT=Path('/code/aster/.wt-deepseek-platform-runtime')
POWER=ROOT/'apps/cloud/crates/power'
COPY=ROOT/'crates/power'

def write(relative, data):
    for base in [POWER,COPY]:(base/relative).write_text(data)

p=POWER/'src/backend/xolotl/prepare.rs'
s=p.read_text()
begin=s.index('        if let Some(dspark) = dspark {')
end=s.index('        assembly.prepare()',begin)
body=s[begin:end]
inner=body[body.index('            dspark.validate()?;'):body.rindex('        }')]
function='fn verify_dspark_sidecar(dspark: crate::config::XolotlDSparkConfig, assembly: &Assembly) -> Result<()> {\n'+inner+'    Ok(())\n}\n'
s=s[:begin]+'        if let Some(dspark) = dspark { verify_dspark_sidecar(dspark, &assembly)?; }\n'+s[end:]
s=s.replace('pub(crate) async fn prepare_managed_model(',function+'pub(crate) async fn prepare_managed_model(',1)
write('src/backend/xolotl/prepare.rs',s)

write('src/backend/xolotl/plan_dspark_tests.rs','''use super::*;
use crate::backend::test_utils::sample_manifest;
use crate::config::{XolotlDSparkConfig, XolotlManagedConfig, XolotlTuning};
fn fixture() -> (PowerConfig, ModelManifest, Vec<CudaDevice>) {
    let mut c=PowerConfig::default();
    c.xolotl.enabled=true;c.xolotl.bin=Some("/runtime/ds41_generate".into());
    c.xolotl.model=Some("/models/DeepSeek-V4.1-Flash-1017-XQ.gguf".into());
    c.xolotl.engram=Some("/models/engram".into());c.xolotl.rank=Some("/models/rank.bin".into());
    c.xolotl.gpus=vec![6];c.xolotl.layer_split="none".into();c.xolotl.cuda_architecture="8.9".into();c.xolotl.kv_fp4=true;
    c.xolotl.tuning=Some(XolotlTuning{context_tokens:1048576,cpu_threads:24,arena_gib:144,prompt_batch:8192,prefill_chunk:512,pcie_bp:0,tier_batch_pcie_bp:10000});
    c.xolotl.managed=Some(XolotlManagedConfig{model_name:"xq".into(),model_sha256:"a".repeat(64),model_parts:vec![],gpu_uuids:vec![],python:"/python".into(),bridge:"/bridge".into(),server:"/server".into(),tokenizer:"/tokenizer".into(),startup_timeout_seconds:1800,shutdown_timeout_seconds:15});
    c.xolotl.dspark=Some(XolotlDSparkConfig{sidecar:"/models/draft.gguf".into(),sidecar_sha256:"b".repeat(64),depth:4,confidence_bp:6000,evict_prompt_tokens:4096});
    let mut m=sample_manifest("xq");m.path=c.xolotl.model.clone().unwrap().into();
    let gpu=parse_nvidia_smi_csv("6, NVIDIA GeForce RTX 4090 D, 8.9, 24564\\n").unwrap();
    (c,m,gpu)
}
#[test]
fn dspark_plan_preserves_exact_routing_one_gpu_and_one_million_context() {
    let (c,m,g)=fixture();let p=plan(&c,&m,&g).unwrap();
    assert_eq!(p.nvidia_indices,vec![6]);assert_eq!(arg_value(&p.argv,"--ctx").unwrap(),"1048576");
    for key in ["--route-bias","--route-bias-file","--route-bias-remote"]{assert_eq!(arg_value(&p.argv,key).unwrap(),"0");}
    assert_eq!(p.env.iter().find(|(k,_)|k=="STRATA_DSPARK_DEPTH").unwrap().1,"4");
    assert_eq!(p.env.len(),5);
}
#[test]
fn dspark_refuses_unowned_or_unproven_shapes() {
    for mode in ["managed","tuning","kv","placement","depth"] {
        let (mut c,m,g)=fixture();match mode {
            "managed"=>c.xolotl.managed=None,"tuning"=>c.xolotl.tuning=None,
            "kv"=>c.xolotl.kv_fp4=false,"placement"=>c.xolotl.layer_split=REQUIRED_LAYER_SPLIT.into(),
            "depth"=>c.xolotl.dspark.as_mut().unwrap().depth=6,_=>unreachable!(),
        }
        assert!(plan(&c,&m,&g).is_err(),"{mode}");
    }
}
''')
plan=(POWER/'src/backend/xolotl/plan.rs').read_text()
plan+='\n#[cfg(test)]\n#[path = "plan_dspark_tests.rs"]\nmod dspark_tests;\n'
write('src/backend/xolotl/plan.rs',plan)

s=(POWER/'src/backend/xolotl/prepare_tests.rs').read_text()
s+='''
#[test]
fn dspark_binding_refuses_corruption_missing_magic_and_symlink() {
    for bad in ["none","digest","magic","symlink"] {
        let root=tempfile::tempdir().unwrap();let a=fixture(root.path());
        let path=root.path().join("draft.gguf");let bytes=b"GGUF-owned-draft";std::fs::write(&path,bytes).unwrap();
        let mut d=crate::config::XolotlDSparkConfig{sidecar:path.clone(),sidecar_sha256:sha(bytes),depth:4,confidence_bp:6000,evict_prompt_tokens:4096};
        match bad {
            "digest"=>d.sidecar_sha256="f".repeat(64),
            "magic"=>{let b=b"NOPE-owned-draft";std::fs::write(&path,b).unwrap();d.sidecar_sha256=sha(b);},
            "symlink"=>{let target=root.path().join("original");std::fs::rename(&path,&target).unwrap();std::os::unix::fs::symlink(target,&path).unwrap();},
            "none"=>{},_=>unreachable!(),
        }
        assert_eq!(verify_dspark_sidecar(d,&a).is_ok(),bad=="none","{bad}");
        assert!(!a.path.exists());
    }
}
'''
write('src/backend/xolotl/prepare_tests.rs',s)

test=ROOT/'tools/model-runtime/deepseek-v41-xq/test_power_xolotl_bridge.py'
s=test.read_text()
marker='class Boundary(unittest.TestCase):\n'
assert s.count(marker)==1
tests='''class Boundary(unittest.TestCase):
    def test_dspark_environment_is_complete_bounded_and_single_card(self):
        value=launch()
        draft={"STRATA_SERVE_DSPARK":"/models/draft.gguf","STRATA_DSPARK_DEPTH":"4","STRATA_DSPARK_CONF":"0.6000","STRATA_SERVE_DSPARK_EVICT":"4096"}
        value["engine_env"].update(draft)
        self.assertEqual(decode(value),value)
        for key, bad in [("STRATA_DSPARK_DEPTH","6"),("STRATA_DSPARK_DEPTH","04"),("STRATA_DSPARK_CONF","1.0001"),("STRATA_SERVE_DSPARK_EVICT","1"),("STRATA_SERVE_DSPARK","relative.gguf")]:
            v=copy.deepcopy(value);v["engine_env"][key]=bad
            with self.subTest(key=key,bad=bad),self.assertRaises(ValueError):decode(v)
        v=copy.deepcopy(value);del v["engine_env"]["STRATA_DSPARK_CONF"]
        with self.assertRaises(ValueError):decode(v)
        v=copy.deepcopy(value);v["engine_env"]["LD_PRELOAD"]="/evil.so"
        with self.assertRaises(ValueError):decode(v)

    def test_dspark_requires_success_in_current_process_log_not_stale_success(self):
        with tempfile.TemporaryDirectory() as root:
            log=Path(root)/"engine.log"
            good=b"\\nDSpark: /models/draft.gguf, 2.90 GB of weights on the card (1.0 s)\\n"
            log.write_bytes(good);boundary=log.stat().st_size
            env={"STRATA_SERVE_DSPARK":"/models/draft.gguf"}
            with self.assertRaises(ValueError):bridge.dspark_proof(log,boundary,env)
            with log.open("ab") as stream:stream.write(good)
            self.assertTrue(bridge.dspark_proof(log,boundary,env))
            self.assertFalse(bridge.dspark_proof(log,boundary,{}))
            target=Path(root)/"target";log.rename(target);log.symlink_to(target)
            with self.assertRaises(OSError):bridge.dspark_proof(log,0,env)

'''
s=s.replace(marker,tests,1);test.write_text(s)
print('Sidecar tamper/symlink, canonical ACL/environment, exact plan and current-process activation proofs added')
