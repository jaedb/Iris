build_hotdoc = false

if meson.is_cross_build()
    if get_option('doc').enabled()
        error('Documentation enabled but building the doc while cross building is not supported yet.')
    endif

    message('Documentation not built as building it while cross building is not supported yet.')
    subdir_done()
endif

if default_library == 'static'
    if get_option('doc').enabled()
        error('Documentation enabled but not supported when building statically.')
    endif

    message('Building statically, can\'t build the documentation')
    subdir_done()
endif

if gst_dep.type_name() == 'internal'
    gst_proj = subproject('gstreamer')
    plugins_cache_generator = gst_proj.get_variable('plugins_cache_generator')
else
    plugins_cache_generator = find_program(join_paths(gst_dep.get_variable('libexecdir'), 'gstreamer-1.0' , 'gst-plugins-doc-cache-generator'),
        required: false)
    if not plugins_cache_generator.found()
      plugins_cache_generator = find_program('gst-plugins-doc-cache-generator',
          dirs: [join_paths(gst_dep.get_variable('libexecdir', default_value: ''), 'gstreamer-' + api_version)],
          required: false)
    endif
endif

libs_doc = []
plugins_cache = join_paths(meson.current_source_dir(), 'plugins', 'gst_plugins_cache.json')
if plugins.length() == 0
    message('All base plugins have been disabled')
elif plugins_cache_generator.found()
    plugins_paths = []
    foreach plugin: plugins
      plugins_paths += [plugin.full_path()]
    endforeach
    # We do not let gstreamer update our cache
    _plugins_doc_dep = custom_target('rs-plugins-doc-cache',
        command: [plugins_cache_generator, plugins_cache, '@OUTPUT@', plugins_paths],
        input: plugins,
        output: 'gst_plugins_cache.json',
        build_always_stale: true,
    )
else
    warning('GStreamer plugin inspector for documentation not found, can\'t update the cache')
endif

if get_option('doc').disabled()
  subdir_done()
endif

hotdoc_p = find_program('hotdoc', required: get_option('doc'))
if not hotdoc_p.found()
    message('Hotdoc not found, not building the documentation')
    subdir_done()
endif

hotdoc_req = '>= 0.11.0'
hotdoc_version = run_command(hotdoc_p, '--version', check: false).stdout()
if not hotdoc_version.version_compare(hotdoc_req)
    if get_option('doc').enabled()
        error('Hotdoc version @0@ not found, got @1@'.format(hotdoc_req, hotdoc_version))
    else
        message('Hotdoc version @0@ not found, got @1@'.format(hotdoc_req, hotdoc_version))
        subdir_done()
    endif
endif

hotdoc = import('hotdoc')
required_hotdoc_extensions = ['gst-extension']
foreach extension: required_hotdoc_extensions
    if not hotdoc.has_extensions(extension)
        if get_option('doc').enabled()
            error('Documentation enabled but @0@ missing'.format(extension))
        endif

        message('@0@ extension not found, not building documentation'.format(extension))
        subdir_done()
    endif
endforeach

build_hotdoc = true
cdir = meson.current_source_dir()
gst_plugins_doc = run_command(
    plugins_cache_generator,
    'hotdoc-config',
    '--builddir', meson.current_build_dir(),
    '--project_version', '1.0',
    '--sitemap', cdir / 'plugins/sitemap.txt',
    '--index', cdir / 'plugins/index.md',
    '--include_paths', meson.current_source_dir() / '..',
    '--gst_index', cdir / 'plugins/index.md',
    '--gst_c_source_filters', cdir / '../target/*/*.rs',
    '--gst_c_source_filters',
      cdir / '../target/*/*/*.rs',
      cdir / '../target/*/*/*/*.rs',
      cdir / '../target/*/*/*/*/*.rs',
      cdir / '../target/*/*/*/*/*/*.rs',
    '--gst_c_sources',
      cdir / '../*/*/*/*.rs',
      cdir / '../*/*/*/*/*.rs',
      cdir / '../*/*/*/*/*/*.rs',
    '--gst_cache_file', plugins_cache,
    '--extra_assets', join_paths(meson.current_source_dir(), 'images'),
    check: true,
).stdout().split(pathsep)
