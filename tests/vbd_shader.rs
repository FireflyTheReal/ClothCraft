#[test]
fn contact_bvh_validates_without_a_gpu() {
    let source = format!(
        "{}\n{}",
        include_str!("../src/gpu_vbd/contact_bvh.wgsl"),
        include_str!("../src/gpu_vbd/contact_geometry.wgsl")
    );
    let module = naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap();
}

#[test]
fn vbd_compute_shader_validates_without_a_gpu() {
    let source = format!(
        "{}\n{}",
        include_str!("../src/vbd.wgsl"),
        include_str!("../src/gpu_vbd/reduce_portable.wgsl")
    );
    let module = naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap();
}

#[test]
fn subgroup_reductions_validate_without_a_gpu() {
    let source = format!(
        "{}\n{}",
        include_str!("../src/vbd.wgsl").replace(
            "@builtin(local_invocation_index)",
            "@builtin(subgroup_invocation_id)",
        ),
        include_str!("../src/gpu_vbd/reduce_subgroup.wgsl")
    );
    let module = naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&source)));
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::SUBGROUP,
    );
    validator.subgroup_stages(naga::valid::ShaderStages::COMPUTE);
    validator.subgroup_operations(naga::valid::SubgroupOperationSet::all());
    validator.validate(&module).unwrap();
}
