plugins = []
rs_plugins = []
if output.length() > 0
  rs_plugins = custom_target('gst-plugins-rs',
    build_by_default: true,
    output: output,
    console: true,
    install: true,
    install_dir: install_dirs,
    depends: depends,
    depfile: 'gst-plugins-rs.dep',
    env: extra_env,
    command: [cargo_wrapper,
      'build',
      meson.current_build_dir(),
      meson.current_source_dir() / '..',
      meson.global_build_root(),
      target,
      get_option('prefix'),
      get_option('libdir'),
      '--packages', packages,
      '--depfile', '@DEPFILE@',
      '--lib-suffixes', library_suffixes,
      '--target-dir', meson.current_build_dir() / '..' / 'target',
    ] + feature_args + extra_args + (get_option('tests').allowed() ? ['--build-tests'] : []))
  plugins = []
  foreach plugin: rs_plugins.to_list()
    if not plugin.full_path().endswith('.pc')
      plugins += plugin
    endif
  endforeach
endif

run_target('clippy',
  env: extra_env,
  command: [cargo_wrapper,
    'clippy',
    meson.current_build_dir(),
    meson.current_source_dir() / '../',
    meson.global_build_root(),
    target,
    get_option('prefix'),
    get_option('libdir'),
    '--packages', packages,
    '--lib-suffixes', library_suffixes,
  ] + feature_args + extra_args)
