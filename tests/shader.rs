/// Parse and validate WGSL entirely on CPU. Never create a wgpu Instance or adapter.
#[test]
fn viewport_shader_validates_without_gpu() {
    let module = naga::front::wgsl::parse_str(include_str!("../src/cloth.wgsl")).unwrap();
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap();
}
