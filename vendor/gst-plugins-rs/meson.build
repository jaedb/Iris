project('gst-plugins-rs',
        'rust',
        'c',
        version: '0.15.0-alpha.1',
        meson_version : '>= 1.1')

# dependencies.py needs a toml parsing module
api_version = '1.0'
python = import('python').find_installation(modules: ['tomllib'], required: false)
if not python.found()
  python = import('python').find_installation(modules: ['tomli'])
endif
fs = import('fs')
host_system = host_machine.system()

if get_option('debug')
  target = 'debug'
else
  target = 'release'
endif

cargo = find_program('cargo', version:'>=1.40')
cargo_wrapper = find_program('cargo_wrapper.py')
cargo_c = find_program('cargo-cbuild', version:'>=0.9.21', required: false)

rustc = meson.get_compiler('rust')

if not cargo_c.found()
  error('cargo-c missing, install it with: \'cargo install cargo-c\'')
endif

system = host_machine.system()
exe_suffix = ''
if system == 'windows'
  exe_suffix = '.exe'
  ext_dynamic = 'dll'
  ext_static = 'lib'
elif system == 'darwin'
  ext_dynamic = 'dylib'
  ext_static = 'a'
else
  ext_dynamic = 'so'
  ext_static = 'a'
endif

# Extra env to pass to cargo
extra_env = {}

# Used to not lookup the same dependency multiple times which clutters logs
deps_cache = {}

# Need to depends on all gstreamer-rs deps to ensure they are built
# before gstreamer-rs when building with gst-build.
# Custom targets can't depend on dependency() objects so we have to depend
# on the library variable from the subproject instead.
glib_req = '>=2.62'
gst_req = '>=1.20.0'
depends = []

# kept in the same order as the `members` list in Cargo.toml
plugins = {
  'analytics': {
    'library': 'libgstrsanalytics',
    'features': ['gst-rtp/v1_24', 'gst-base/v1_24', 'gst-video/v1_24'],
  },
  'audiofx': {
    'library': 'libgstrsaudiofx',
    'examples': ['hrtfrender'],
  },
  'audioparsers': {
    'library': 'libgstrsaudioparsers',
  },
  'claxon': {'library': 'libgstclaxon'},
  # csound has a non-trivial external dependency, see below
  'csound': {},
  'lewton': {'library': 'libgstlewton'},
  'spotify': {'library': 'libgstspotify'},

  'file': {'library': 'libgstrsfile'},
  'originalbuffer': {'library': 'libgstoriginalbuffer'},
  # sodium can have an external dependency, see below
  'sodium': {},
  'threadshare': {
    'library': 'libgstthreadshare',
    'examples': [
      'ts-benchmark',
      'udpsrc-benchmark-sender',
      'tcpclientsrc-benchmark-sender',
      'ts-standalone',
    ],
  },
  'inter': {'library': 'libgstrsinter'},
  'streamgrouper': {'library': 'libgststreamgrouper'},

  'isobmff': {
    'library': 'libgstisobmff',
    'examples': [
      'dash_vod',
      'hls_live',
      'hls_vod',
    ],
  },

  'aws': {
    'library': 'libgstaws',
    'extra-deps': {'openssl': ['>=1.1']},
  },
  'deepgram': {'library': 'libgstdeepgram'},
  'mpegtslive': {'library': 'libgstmpegtslive'},
  'hlsmultivariantsink': {'library': 'libgsthlsmultivariantsink'},
  'hlssink3': {'library': 'libgsthlssink3'},
  'ndi': {'library': 'libgstndi'},
  'onvif': {
    'library': 'libgstrsonvif',
    'extra-deps': {'pangocairo': []},
  },
  'raptorq': {'library': 'libgstraptorq'},
  'reqwest': {'library': 'libgstreqwest'},
  'rtsp': {'library': 'libgstrsrtsp'},
  'rtp': {'library': 'libgstrsrtp'},
  'webrtchttp': {'library': 'libgstwebrtchttp'},
  'webrtc': {
    'library': 'libgstrswebrtc',
    'examples': ['webrtcsink-stats-server'],
    'option-features': ['aws', 'livekit']
  },

  'textaccumulate': {'library': 'libgsttextaccumulate'},
  'textahead': {'library': 'libgsttextahead'},
  'json': {'library': 'libgstjson'},
  'regex': {'library': 'libgstregex'},
  'textwrap': {'library': 'libgsttextwrap'},

  'fallbackswitch': { }, # fallbackswitch can have gtk4 examples, see below
  'livesync': { },     # livesync can have gtk4 examples, see below
  'togglerecord': { }, # togglerecord can have gtk4 examples, see below

  'tracers': {'library': 'libgstrstracers'},
  'uriplaylistbin': {
    'library': 'libgsturiplaylistbin',
    'examples': ['playlist'],
    'features': ['clap'],
    'gst-version': '>=1.23.90',
  },

  'cdg': {'library': 'libgstcdg'},
  'closedcaption': {
    'library': 'libgstrsclosedcaption',
    'extra-deps': {
      'pango': [],
      'pangocairo': [],
      'cairo-gobject': [],
    }
  },
  'dav1d': {
    'library': 'libgstdav1d',
    'extra-deps': {'dav1d': ['>=1.3']},
    'features': ['gst/v1_18', 'gst-base/v1_18', 'gst-video/v1_18'],
  },
  'elevenlabs': {'library': 'libgstelevenlabs'},
  'ffv1': {'library': 'libgstffv1'},
  'flavors': {'library': 'libgstrsflv'},
  'gif': {
    'library': 'libgstgif',
    'examples': ['testvideosrc2gif'],
  },
  # gtk4 is re added below
  'gtk4': {},
  'hsv': {'library': 'libgsthsv'},
  'png': {
    'library': 'libgstrspng',
    'examples': ['pngenc'],
  },
  'rav1e': {'library': 'libgstrav1e'},
  'videofx': {
    'library': 'libgstrsvideofx',
    'extra-deps': {'cairo-gobject': []},
  },
  'gopbuffer': {'library': 'libgstgopbuffer'},
  'quinn': {
    'library': 'libgstquinn',
    'examples': [
      'quic_mux',
      'quic_roq',
    ],
  },
  'skia': {}, # skia can have external dependencies, see below
  'speechmatics': {'library': 'libgstspeechmatics'},
  'vvdec': {
    'library': 'libgstvvdec',
    'extra-deps': {'libvvdec': ['>= 3.0']}
  },
  'webp': {}, # webp has external dependencies, see below
}

# Convert auto options to use auto_plugin_features for plugin-related options
# so that we can change the auto behavior in one place
auto_plugin_features = get_option('auto_plugin_features')
foreach option_name: plugins.keys()
  opt = get_option(option_name)
  if opt.auto()
    opt = auto_plugin_features
  endif
  set_variable(f'@option_name@_option', opt)
endforeach

rtp_option = rtp_option.enable_if(webrtc_option.enabled(), error_message: 'webrtc option needs rtp')
rtp_option = rtp_option.enable_if(rtsp_option.enabled(), error_message: 'rtsp option needs rtp')

deps = [
  # name, subproject name, subproject dep, library object
  ['gstreamer-1.0', 'gstreamer', 'gst_dep', 'libgst'],
  ['gstreamer-app-1.0', 'gst-plugins-base', 'app_dep', 'gstapp'],
  ['gstreamer-audio-1.0', 'gst-plugins-base', 'audio_dep', 'gstaudio'],
  ['gstreamer-base-1.0', 'gstreamer', 'gst_base_dep', 'gst_base'],
  ['gstreamer-video-1.0', 'gst-plugins-base', 'video_dep', 'gstvideo'],
]

if threadshare_option.allowed() \
  or onvif_option.allowed() \
  or raptorq_option.allowed() \
  or rtp_option.allowed() \
  or webrtc_option.allowed()
  req = threadshare_option.enabled() or onvif_option.enabled() \
      or raptorq_option.enabled() or rtp_option.enabled() or webrtc_option.enabled()
  deps += [['gstreamer-rtp-1.0', 'gst-plugins-base', 'rtp_dep', 'gst_rtp', req]]
endif
if webrtc_option.allowed() \
  or webrtchttp_option.allowed()
  req = webrtc_option.enabled() or webrtchttp_option.enabled()
  deps += [['gstreamer-webrtc-1.0', 'gst-plugins-bad', 'gstwebrtc_dep', 'gstwebrtc', req]]
  deps += [['gstreamer-sdp-1.0', 'gst-plugins-base', 'sdp_dep', 'gstsdp', req]]
endif
if get_option('tests').allowed()
  deps += [['gstreamer-check-1.0', 'gstreamer', 'gst_check_dep', 'gst_check', get_option('tests')]]
endif
if gtk4_option.allowed()
  deps += [['gstreamer-gl-1.0', 'gst-plugins-base', 'gstgl_dep', 'gstgl', gtk4_option]]
endif
if threadshare_option.allowed() or rtsp_option.allowed()
  req = threadshare_option.enabled() or rtsp_option.enabled()
  deps += [['gstreamer-net-1.0', 'gstreamer', 'gst_net_dep', 'gst_net', req]]
endif
if analytics_option.allowed()
  deps += [['gstreamer-analytics-1.0', 'gst-plugins-bad', 'gstanalytics_dep', 'gstanalytics', analytics_option]]
endif

glib_dep = dependency('glib-2.0', version: glib_req)
deps_cache += {'glib-2.0': glib_dep}

foreach d: deps
  dep = dependency(d[0], version: gst_req,
    fallback : [d[1], d[2]], required: d.get(4, true))
  set_variable(d[2], dep)
  deps_cache += {d[0]: dep}
  if dep.type_name() == 'internal'
    lib = subproject(d[1]).get_variable(d[3])
    depends += lib
  endif
endforeach

# The splitter/combiner requires 1.28+
if analytics_option.allowed()
  gst_analytics_dep = deps_cache['gstreamer-analytics-1.0']
  if gst_analytics_dep.found() and gst_analytics_dep.version().version_compare('>= 1.28')
    plugins_analytics = plugins['analytics']
    plugins_analytics += {
      'features': plugins_analytics['features'] + ['v1_28'],
    }
    plugins += {
      'analytics': plugins_analytics
    }
  endif
endif

# Won't build on platforms where it bundles the sources because of:
# https://github.com/qnighy/libwebp-sys2-rs/issues/12
# the fix is:
# https://github.com/qnighy/libwebp-sys2-rs/pull/13
if host_system not in ['windows', 'darwin']
  # FIXME: libwebp-sys2 will build its bundled version on msvc and apple platforms
  # https://github.com/qnighy/libwebp-sys2-rs/issues/4
  plugins += {'webp': {
    'library': 'libgstrswebp',
    'extra-deps': {'libwebpdemux': []},
  }}
endif

if sodium_option.allowed()
  sodium_plugin = {'sodium': {
    'library': 'libgstsodium',
    'examples': ['generate-keys', 'encrypt-example', 'decrypt-example'],
    'features': ['serde', 'serde_json', 'clap'],
  }}
  if get_option('sodium-source') == 'system'
    sodium_dep = dependency('libsodium', required: sodium_option.enabled())
    extra_env += {'SODIUM_USE_PKG_CONFIG': '1'}
    if sodium_dep.found()
      plugins += sodium_plugin
    endif
  else
    plugins += sodium_plugin
  endif
endif

cc = meson.get_compiler('c')
if csound_option.allowed()
  # if csound isn't distributed with pkg-config then user needs to define CSOUND_LIB_DIR with its location

  res = run_command(python, '-c', 'import os; print(os.environ["CSOUND_LIB_DIR"])', check: false)
  if res.returncode() == 0
    csound_libdir = res.stdout().strip()
    csound_dep = cc.find_library('csound64', dirs: csound_libdir, required: false)
    if csound_dep.found()
      plugins += {'csound': {
        'library': 'libgstcsound',
        'examples': ['csound-effect'],
      }}
      extra_env += {'CSOUND_LIB_DIR': csound_libdir}
    elif csound_option.enabled()
      error('csound option is enabled, but csound64 library could not be found and CSOUND_LIB_DIR was not set')
    endif
  endif
endif

if gtk4_option.allowed()
  gtk4_features = []
  gl_winsys = gstgl_dep.get_variable('gl_winsys').split()
  gl_platforms = gstgl_dep.get_variable('gl_platforms').split()
  if 'wayland' in gl_winsys
    gtk4_features += 'waylandegl'
  endif
  if 'x11' in gl_winsys
    if 'egl' in gl_platforms
      gtk4_features += 'x11egl'
    endif
    if 'glx' in gl_platforms
      gtk4_features += 'x11glx'
    endif
  elif host_system == 'windows'
    if 'egl' in gl_platforms
      gtk4_features += 'winegl'
    endif
  endif

  gst_allocators_dep = dependency('gstreamer-allocators-1.0', version: '>=1.24', required: false)
  gtk_dep = dependency('gtk4', version: '>=4.6', required: gtk4_option)
  if gtk_dep.found()
    if host_system == 'linux' and gtk_dep.version().version_compare('>=4.14') and \
        gst_allocators_dep.found() and 'waylandegl' in gtk4_features
      gtk4_features += 'dmabuf'
    endif

    plugins += {
      'gtk4': {
        'library': 'libgstgtk4',
        'examples': ['gtksink'],
        'extra-deps': {'gtk4': ['>=4.6']},
        'features': gtk4_features,
      },
    }
  endif
endif

examples_opt = get_option('examples')
if examples_opt.allowed() and 'gtk4' in plugins
  plugins += {
    'fallbackswitch': {
      'library': 'libgstfallbackswitch',
      'examples_features': {
        'gtk-fallbackswitch': ['gtk', 'gio', 'gst-plugin-gtk4'],
      },
    },
    'livesync': {
      'library': 'libgstlivesync',
      'examples_features': {
        'gtk-livesync': ['gtk', 'gio', 'gst-plugin-gtk4'],
      }
    },
    'togglerecord': {
      'library': 'libgsttogglerecord',
      'examples_features': {
        'gtk-recording': ['gtk', 'gio', 'gst-plugin-gtk4'],
      }
    },
  }
else
  plugins += {
    'fallbackswitch': { 'library': 'libgstfallbackswitch'},
    'livesync': { 'library': 'libgstlivesync'},
    'togglerecord': { 'library': 'libgsttogglerecord'},
  }
endif

if skia_option.allowed()
  # Add harfbuzz and freetype dependencies for skia plugin on Linux
  if host_system == 'linux'
    skia_extra_deps = {'harfbuzz': [], 'freetype2': [], 'fontconfig': []}
  else
    skia_extra_deps = {}
  endif

  plugins += {
    'skia': {
      'library': 'libgstskia',
      'extra-deps': skia_extra_deps,
    },
  }
endif

# Process plugins list

default_library = get_option('default_library')

library_suffixes = []
if default_library in ['shared', 'both']
  library_suffixes += [ext_dynamic]
endif
if default_library in ['static', 'both']
  library_suffixes += [ext_static]
endif

# cargo packages (plugins) to build
packages = []
# cargo features
features = []
# examples to build
examples = []
# Add the plugin library and .pc files as output
output = []
# List of directories to install output files into ([plugin_install_dir,
# pkgconfig_install_dir, ...])
install_dirs = []

# List of features from meson dependencies
found_features = {}
# List of dependencies for each plugin from Cargo.toml
feature_deps = {}
# List of dependencies that are actually enabled
required_feature_deps = []

glib_versions = ['2.74','2.72','2.70','2.68','2.66','2.64','2.62','2.60','2.58']
foreach glib_version : glib_versions
  if glib_dep.version().version_compare(f'>=@glib_version@')
    found_features += {'glib': 'glib/v' + glib_version.underscorify()}
    found_features += {'gio': 'gio/v' + glib_version.underscorify()}
    break
  endif
endforeach

if gtk4_option.allowed() and gtk_dep.found()
  gtk4_versions = ['4.20','4.18','4.16','4.14','4.12','4.10']
  foreach gtk4_version : gtk4_versions
    if gtk_dep.version().version_compare(f'>=@gtk4_version@')
      found_features += {'gtk': 'gtk_v' + gtk4_version.underscorify()}
      break
    endif
  endforeach
endif

p = run_command('dependencies.py', meson.current_source_dir(), '--feature-deps', capture: true, check: true)
foreach line : p.stdout().split('\n')
  if ':' in line
    tmp = line.split(':')
    library_name = 'lib' + tmp[0].strip()
    feature_deps += {library_name: tmp[1].strip().split(',')}
  endif
endforeach

if rav1e_option.allowed()
  nasm = find_program('nasm', required: false)
  if nasm.found()
    features += 'gst-plugin-rav1e/asm'
    extra_env += {'NASM': nasm.full_path()}
  endif
endif

if get_option('default_library') == 'static'
  extra_env += {
    # Tell the pkg-config crate to think of all libraries as static
    'PKG_CONFIG_ALL_STATIC': '1',
    # Tell the system-deps crate to process linker flag for static deps
    'SYSTEM_DEPS_LINK': 'static'
  }
endif

plugins_install_dir = get_option('libdir') / 'gstreamer-1.0'
pkgconfig_install_dir = plugins_install_dir / 'pkgconfig'
disable_pc_generation = get_option('default_library') == 'shared'

foreach plugin_name, details: plugins
  if details == {}
    # Some plugins (e.g., csound, sodium, gtk4, fallbackswitch, livesync, togglerecord)
    # are initially defined with empty {} and only get their full details added later
    # if their external dependencies (like gtk4, csound library, etc.) are found.
    # Skip plugins that haven't been filled in yet.
    continue
  endif

  plugin_opt = get_variable(f'@plugin_name@_option')
  if not plugin_opt.allowed()
    debug(f'@plugin_name@ is disabled')
    continue
  endif
  plugin_deps_found = true

  # Check whether we have all needed deps
  foreach dep_name, dep_ver: details.get('extra-deps', {})
    if dep_ver.length() != 0
      dep = dependency(dep_name, version: dep_ver, required: plugin_opt)
    else
      dep = dependency(dep_name, required: plugin_opt)
    endif
    deps_cache += {dep_name: dep}
    if not dep.found()
      if dep_ver.length() != 0
        dep_ver_msg = ' '.join(dep_ver)
        debug(f'@plugin_name@ dependency @dep_name@ @dep_ver_msg@ not found, skipping')
      else
        debug(f'@plugin_name@ dependency @dep_name@ not found, skipping')
      endif
      plugin_deps_found = false
      break
    endif
  endforeach
  if not plugin_deps_found
    continue
  endif

  # Build list of required features
  library_name = details.get('library', [])
  if feature_deps.has_key(library_name)
    foreach d : feature_deps.get(library_name)
      if d not in required_feature_deps
        required_feature_deps += d
      endif
    endforeach
  endif

  # Validate gst-plugin features
  plugin_features = details.get('features', [])
  foreach feature: plugin_features
    if feature.startswith('gst-plugin') and not packages.contains(feature)
      msg = f'@plugin_name@ required feature @feature@ not found'
      if plugin_opt.enabled()
        error(msg)
      endif
      message(msg + ', skipping')
      plugin_deps_found = false
      break
    endif
  endforeach
  if not plugin_deps_found
    continue
  endif

  option_features = []
  foreach feature : details.get('option-features', [])
    if get_option(f'@plugin_name@-@feature@').allowed()
      message(f'Enabling @feature@ for @plugin_name@')
      features += [feature]
    endif
  endforeach

  # Check if we have the required GStreamer version
  if details.has_key('gst-version') and not \
      deps_cache['gstreamer-1.0'].version().version_compare(details['gst-version'])
    msg = '@0@ requires gstreamer version @1@'.format(plugin_name, details['gst-version'])
    if plugin_opt.enabled()
      error(msg)
    endif
    message(msg + ', skipping')
    continue
  endif

  # Parse and enable examples
  plugin_examples = details.get('examples', [])
  foreach example: plugin_examples
    examples += example
  endforeach

  plugin_examples_features = details.get('examples_features', {})
  foreach example, examples_features: plugin_examples_features
    example_deps_found = true
    foreach feature: examples_features
      if feature.startswith('gst-plugin') and not packages.contains(feature)
        msg = f'@plugin_name@ example @example@ required feature @feature@ not found'
        if plugin_opt.enabled() and examples_opt.enabled()
          error(msg)
        endif
        message(msg + ', skipping')
        example_deps_found = false
        break
      endif
    endforeach
    features += examples_features
    if example_deps_found
      examples += example
    endif
  endforeach

  packages += f'gst-plugin-@plugin_name@'
  features += plugin_features
  extra_features = run_command('dependencies.py', meson.current_source_dir(), plugin_name,
    '--features', '--gst-version', gst_dep.version(), capture: true, check: true).stdout().strip()
  if extra_features != ''
    features += extra_features.split(',')
  endif

  lib = details.get('library')
  libname = lib.substring(3)
  # No 'lib' suffix with MSVC
  if cc.get_argument_syntax() == 'msvc'
    lib = libname
  endif
  if default_library in ['shared', 'both']
    output += [lib + '.' + ext_dynamic]
    install_dirs += [plugins_install_dir]
  endif
  if default_library in ['static', 'both']
    output += [lib + '.' + ext_static]
    install_dirs += [plugins_install_dir]
  endif

  if not disable_pc_generation
    output += [libname + '.pc']
    install_dirs += [pkgconfig_install_dir]
  endif
endforeach

# Add required features
foreach feat : required_feature_deps
  if found_features.has_key(feat)
    features += found_features[feat]
  endif
endforeach

feature_args = []
if features.length() > 0
  feature_args += ['--features', features]
endif

extra_args = []
if get_option('doc').disabled()
  extra_args += ['--disable-doc']
endif

# 'pkgconfig' is the entry in the machine file, if specified
pkg_config = find_program('pkgconfig', 'pkg-config')
if pkg_config.found()
  extra_env += {'PKG_CONFIG': pkg_config.full_path()}
endif

pathsep = ':'
if host_system == 'windows'
  pathsep = ';'
endif

pkg_config_path = get_option('pkg_config_path')
if pkg_config_path.length() > 0
  extra_env += {'PKG_CONFIG_PATH': pathsep.join(pkg_config_path)}
endif

# get cmdline for rust
extra_env += {'RUSTC': ' '.join(rustc.cmd_array())}

# Build plugins in the plugins subdirectory
subdir('plugins')

# This is used by GStreamer to static link Rust plugins into gst-full
gst_plugins = []
plugin_names = []
foreach plugin : plugins
  plugin_name = fs.stem(plugin.full_path())
  # skip the 'lib' prefix from plugin path when not building with MSVC
  if cc.get_argument_syntax() != 'msvc'
    plugin_name = plugin_name.substring(3)
  endif

  plugin_display_name = plugin_name
  if plugin_name.startswith('gst')
    plugin_display_name = plugin_name.substring(3)
  endif
  if plugin_display_name in plugin_names
    # When default_library=both plugins are duplicated.
    continue
  endif
  plugin_names += plugin_display_name

  option_name = plugin_name.substring(3)
  if option_name.startswith('rs')
    option_name = option_name.substring(2)
  endif
  if option_name == 'flv'
    option_name = 'flavors'
  endif
  if not get_option(option_name).allowed()
    continue
  endif

  # Extract plugin dependencies from their Cargo.toml file
  plugin_deps = []
  p = run_command('dependencies.py', meson.current_source_dir(), plugin_name,
    capture: true,
    check: true)
  foreach dep_name : p.stdout().strip().split(',')
    dep_name_version = dep_name.split('|')
    dep_name = dep_name_version.get(0).strip()
    if dep_name_version.length() > 1
      extras = {'version': dep_name_version.get(1).strip()}
    else
      extras = {}
    endif
    if deps_cache.has_key(dep_name)
      plugin_deps += deps_cache[dep_name]
    else
      dep = dependency(dep_name, required: false, kwargs: extras)
      plugin_deps += dep
      deps_cache += {dep_name: dep}
    endif
  endforeach

  dep = declare_dependency(
    link_with: plugin,
    dependencies: plugin_deps,
    variables: {'full_path': plugin.full_path()},
  )
  meson.override_dependency(plugin_name, dep)

  if default_library == 'static' and plugin_name in ['gstcsound', 'gstthreadshare', 'gstgtk4']
    warning('Static plugin @0@ is known to fail. It will not be included in libgstreamer-full.'.format(plugin_name))
  else
    gst_plugins += dep
  endif
endforeach

subdir('docs')

if webrtc_option.allowed()
  custom_target('gst-webrtc-signalling-server',
    build_by_default: true,
    output: 'gst-webrtc-signalling-server' + exe_suffix,
    console: true,
    install: true,
    install_dir: get_option('bindir'),
    depfile: 'gst-webrtc-signalling-server.dep',
    env: extra_env,
    command: [cargo_wrapper,
      'build',
      meson.current_build_dir(),
      meson.current_source_dir(),
      meson.global_build_root(),
      target,
      get_option('prefix'),
      get_option('libdir'),
      '--depfile', '@DEPFILE@',
      '--bin', 'gst-webrtc-signalling-server',
      '--exe-suffix', exe_suffix,
    ])
endif

if get_option('examples').allowed() and examples.length() > 0
  outputs = []
  foreach example: examples
    outputs += [example + exe_suffix]
  endforeach
  custom_target('gst-plugins-rs-examples',
    build_by_default: true,
    output: outputs,
    console: true,
    install: false,
    depfile: 'gst-plugins-rs-examples.dep',
    env: extra_env,
    command: [cargo_wrapper,
      'build',
      meson.current_build_dir(),
      meson.current_source_dir(),
      meson.global_build_root(),
      target,
      get_option('prefix'),
      get_option('libdir'),
      '--depfile', '@DEPFILE@',
      '--packages', packages,
      '--examples', examples,
      '--exe-suffix', exe_suffix,
    ] + feature_args)
endif

if get_option('tests').allowed()
  foreach package: packages
    test(package.replace('gst-plugin-', ''),
      cargo_wrapper,
      env: extra_env,
      args: ['test',
        meson.current_build_dir(),
        meson.current_source_dir(),
        meson.global_build_root(),
        target,
        get_option('prefix'),
        get_option('libdir'),
        '--packages', package],
      timeout: 600)
  endforeach
endif

summary({
    'Plugins': plugin_names,
}, list_sep: ', ')
