pub fn acp_binary() -> String {
    std::env::var_os("A3S_CODE_ACP_BIN")
        .map(|path| path.into_string().expect("A3S_CODE_ACP_BIN must be UTF-8"))
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_a3s-code-acp").to_owned())
}
