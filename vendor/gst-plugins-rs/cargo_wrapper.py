#!/usr/bin/env python3

import glob
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import time
from argparse import ArgumentParser
from pathlib import Path as P

PARSER = ArgumentParser()
PARSER.add_argument('command', choices=['build', 'test', 'clippy'])
PARSER.add_argument('build_dir', type=P)
PARSER.add_argument('src_dir', type=P)
PARSER.add_argument('root_dir', type=P)
PARSER.add_argument('target', choices=['release', 'debug'])
PARSER.add_argument('prefix', type=P)
PARSER.add_argument('libdir', type=P)
PARSER.add_argument('--version', default=None)
PARSER.add_argument('--bin', default=None, type=P)
PARSER.add_argument('--features', nargs='+', default=[])
PARSER.add_argument('--packages', nargs='+', default=[])
PARSER.add_argument('--examples', nargs='+', default=[])
PARSER.add_argument('--lib-suffixes', nargs='+', default=[])
PARSER.add_argument('--exe-suffix')
PARSER.add_argument('--depfile', type=P)
PARSER.add_argument('--target-dir', type=P, default=None)
PARSER.add_argument('--disable-doc', action='store_true', default=False)
PARSER.add_argument('--build-tests', action='store_true', default=False)


def shlex_join(args):
    if hasattr(shlex, 'join'):
        return shlex.join(args)
    return ' '.join([shlex.quote(arg) for arg in args])


def generate_depfile_for(fpath, build_start_time, logfile=None):
    file_stem = fpath.parent / fpath.stem
    depfile_content = ''
    depfile_path = P(f'{file_stem}.d')

    print(f'Checking {depfile_path}', file=logfile)

    # Skip depfiles that are older than the current build start time
    # This avoids stale depfiles from previous branch builds
    if depfile_path.stat().st_mtime < build_start_time:
        print(
            f'{depfile_path} was not generated in our last run, ignoring', file=logfile
        )
        return depfile_content

    with open(depfile_path, 'r') as depfile:
        for l in depfile.readlines():
            if l.startswith(str(file_stem)):
                # We can't blindly split on `:` because on Windows that's part
                # of the drive letter. Lucky for us, the format of the dep file
                # is one of:
                #
                #   /path/to/output: /path/to/src1 /path/to/src2
                #   /path/to/output:
                #
                # So we parse these two cases specifically
                if l.endswith(':'):
                    output = l[:-1]
                    srcs = ''
                else:
                    output, srcs = l.split(': ', maxsplit=2)

                all_deps = []
                for src in srcs.split(' '):
                    src = src.strip()
                    all_deps.append(src)
                    src = P(src)
                    if src.name in ['lib.rs', 'main.rs']:
                        # `rustc` doesn't take `Cargo.toml` into account
                        # but we need to
                        cargo_toml = src.parent.parent / 'Cargo.toml'
                        print(f'  Looking for {cargo_toml}: ', file=logfile, end='')
                        if cargo_toml.exists():
                            print(f'Found', file=logfile)
                            all_deps.append(str(cargo_toml))
                        else:
                            print(f'Not found', file=logfile)

                depfile_content += f"{output}: {' '.join(all_deps)}\n"

    return depfile_content


# Copied from gst-env.py
# libdir is expanded from option of the same name listed in the `meson
# introspect --buildoptions` output.
SHAREDLIB_REG = re.compile(r'\.so|\.dylib|\.dll')
GSTPLUGIN_FILEPATH_REG_TEMPLATE = r'.*/{libdir}/gstreamer-1.0/[^/]+$'
GSTPLUGIN_FILEPATH_REG = None


def listify(o):
    if isinstance(o, str):
        return [o]
    if isinstance(o, list):
        return o
    raise AssertionError('Object {!r} must be a string or a list'.format(o))


def get_target_install_filename(target, filename):
    '''
    Checks whether this file is one of the files installed by the target
    '''
    basename = os.path.basename(filename)
    if "install_filename" not in target:
        return None

    for install_filename in listify(target['install_filename']):
        if install_filename.endswith(basename):
            return install_filename
    return None


def is_library_target_and_not_plugin(target, filename):
    '''
    Don't add plugins to PATH/LD_LIBRARY_PATH because:
    1. We don't need to
    2. It causes us to exceed the PATH length limit on Windows and Wine
    '''
    if target['type'] != 'shared library':
        return False

    # Check if this output of that target is a shared library
    if not SHAREDLIB_REG.search(filename):
        return False

    # Check if it's installed to the gstreamer plugin location
    install_filename = get_target_install_filename(target, filename)
    if not install_filename:
        return False
    global GSTPLUGIN_FILEPATH_REG
    if GSTPLUGIN_FILEPATH_REG is None:
        GSTPLUGIN_FILEPATH_REG = re.compile(GSTPLUGIN_FILEPATH_REG_TEMPLATE)
    if GSTPLUGIN_FILEPATH_REG.search(install_filename.replace('\\', '/')):
        return False
    return True


def setup_library_paths_for_tests(env, builddir, logfile=None):
    '''Setup library paths for test execution using the same logic as gst-env.py'''
    # Determine library path environment variable based on platform
    if os.name == 'nt':
        lib_path_envvar = 'PATH'
    elif sys.platform == 'darwin':
        lib_path_envvar = 'DYLD_LIBRARY_PATH'
    else:
        lib_path_envvar = 'LD_LIBRARY_PATH'

    if lib_path_envvar is None:
        return

    intro_targets_path = P(builddir) / 'meson-info' / 'intro-targets.json'
    if not intro_targets_path.exists():
        print(f'Warning: {intro_targets_path} does not exist', file=logfile)
        return

    try:
        with open(intro_targets_path, 'r') as f:
            targets = json.load(f)

        paths = set()
        for target in targets:
            for filename in target.get('filename', []):
                root = os.path.dirname(filename)
                if is_library_target_and_not_plugin(target, filename):
                    lib_path = os.path.join(builddir, root)
                    paths.add(lib_path)
                    print(f'  Added library path: {lib_path}', file=logfile)
                else:
                    print(f'  Skipped non-library target: {filename}',
                          file=logfile)

        # Prepend all paths to the environment variable
        existing_path = env.get(lib_path_envvar, '')
        if paths:
            new_paths = list(paths)
            if existing_path:
                new_paths.append(existing_path)
            env[lib_path_envvar] = os.pathsep.join(new_paths)
            print(f'Set {lib_path_envvar} for tests', file=logfile)

    except (json.JSONDecodeError, OSError) as e:
        print(f'Error setting up library paths: {e}', file=logfile)


if __name__ == '__main__':
    opts = PARSER.parse_args()
    logdir = opts.root_dir / 'meson-logs'
    run_clippy = opts.command == "clippy"
    logfile_path = (
        logdir
        / f"{opts.depfile.name.replace('.dep', '') if opts.depfile else f'{P(__file__).parent.name}-{opts.command}'}-cargo-wrapper.log"
    )
    logfile = open(logfile_path, mode='w', buffering=1, encoding='utf-8')

    print(opts, file=logfile)
    # Use explicitly passed target dir or default to build_dir/target
    if opts.target_dir:
        cargo_target_dir = opts.target_dir
    else:
        cargo_target_dir = opts.build_dir / 'target'

    env = os.environ.copy()
    if 'PKG_CONFIG_PATH' in env:
        pkg_config_path = env['PKG_CONFIG_PATH'].split(os.pathsep)
    else:
        pkg_config_path = []
    pkg_config_path.append(str(opts.root_dir / 'meson-uninstalled'))
    env['PKG_CONFIG_PATH'] = os.pathsep.join(pkg_config_path)

    if 'NASM' in env:
        env['PATH'] = os.pathsep.join([os.path.dirname(env['NASM']), env['PATH']])

    rustc_target = None
    if 'RUSTC' in env:
        rustc_cmdline = shlex.split(env['RUSTC'], posix=sys.platform != 'win32')
        # grab target from RUSTFLAGS
        rust_flags = rustc_cmdline[1:] + shlex.split(env.get('RUSTFLAGS', ''))
        if '--target' in rust_flags:
            rustc_target_idx = rust_flags.index('--target')
            _ = rust_flags.pop(rustc_target_idx)  # drop '--target'
            rustc_target = rust_flags.pop(rustc_target_idx)
        env['RUSTFLAGS'] = shlex_join(rust_flags)
        env['RUSTC'] = rustc_cmdline[0]

    features = opts.features
    if opts.command in ['build', 'clippy']:
        cargo_cmd = ['cargo']
        if run_clippy:
            cargo_cmd += ["clippy"]
        elif opts.bin or opts.examples:
            cargo_cmd += ['build']
        else:
            cargo_cmd += ['cbuild']
            if not opts.disable_doc:
                features += ['doc']

        # Build tests if requested
        if opts.build_tests and not (opts.bin or opts.examples):
            cargo_cmd.append('--tests')

        if opts.target == 'release':
            cargo_cmd.append('--release')
    elif opts.command == 'test':
        # Set up library paths for test execution
        setup_library_paths_for_tests(env, opts.root_dir, logfile)

        # Check if cargo-nextest is available
        nextest_path = shutil.which('cargo-nextest')
        if nextest_path is not None:
            cargo_cmd = ['cargo', 'nextest', 'run', '--no-fail-fast', '--no-capture']
        else:
            cargo_cmd = ['cargo', 'test', '--no-fail-fast', '--color=always']
    else:
        print('Unknown command:', opts.command, file=logfile)
        sys.exit(1)

    if rustc_target:
        cargo_cmd += ['--target', rustc_target]
    if features:
        cargo_cmd += ['--features', ','.join(features)]
    cargo_cmd += ['--target-dir', cargo_target_dir]
    cargo_cmd += ['--manifest-path', opts.src_dir / 'Cargo.toml']
    if opts.bin:
        cargo_cmd.extend(['--bin', opts.bin.name])
    else:
        if not opts.examples and not opts.command == 'test' and not run_clippy:
            cargo_cmd.extend([
                '--prefix',
                opts.prefix,
                '--libdir',
                opts.prefix / opts.libdir,
            ])
        for p in opts.packages:
            cargo_cmd.extend(['-p', p])
        for e in opts.examples:
            cargo_cmd.extend(['--example', e])

    def run(cargo_cmd, env):
        env_vars = []
        for key, value in sorted(env.items()):
            env_vars.append(f"\n{key}={shlex.quote(value)} \\")

        full_command = ' '.join(env_vars + ['\n'] + [shlex_join([str(c) for c in cargo_cmd])])
        print(f"Running: {full_command}", file=logfile)
        try:
            subprocess.run(cargo_cmd, env=env, cwd=opts.src_dir, check=True)
        except subprocess.SubprocessError:
            sys.exit(1)

    build_start_time = time.time()
    run(cargo_cmd, env)

    if opts.command == 'build':
        target_dir = cargo_target_dir / '**' / opts.target
        if opts.bin:
            exe = glob.glob(
                str(target_dir / opts.bin) + opts.exe_suffix, recursive=True
            )[0]
            shutil.copy2(exe, opts.build_dir)
            depfile_content = generate_depfile_for(P(exe), build_start_time, logfile)
        else:
            # Copy so files to build dir
            depfile_content = ''
            for suffix in opts.lib_suffixes:
                for f in glob.glob(str(target_dir / f'*.{suffix}'), recursive=True):
                    libfile = P(f)

                    depfile_content += generate_depfile_for(
                        libfile, build_start_time, logfile
                    )

                    copied_file = opts.build_dir / libfile.name
                    try:
                        if copied_file.stat().st_mtime == libfile.stat().st_mtime:
                            print(f'{copied_file} has not changed.', file=logfile)
                            continue
                    except FileNotFoundError:
                        pass

                    print(f'Copying {copied_file}', file=logfile)
                    shutil.copy2(f, opts.build_dir)
            # Copy examples to builddir
            for example in opts.examples:
                example_glob = str(target_dir / 'examples' / example) + opts.exe_suffix
                exe = glob.glob(example_glob, recursive=True)[0]
                shutil.copy2(exe, opts.build_dir)
                depfile_content += generate_depfile_for(
                    P(exe), build_start_time, logfile
                )

        with open(opts.depfile, 'w') as depfile:
            depfile.write(depfile_content)

        # Copy generated pkg-config files
        for f in glob.glob(str(target_dir / '*.pc'), recursive=True):
            shutil.copy(f, opts.build_dir)

        # Move -uninstalled.pc to meson-uninstalled
        uninstalled = opts.build_dir / 'meson-uninstalled'
        os.makedirs(uninstalled, exist_ok=True)

        for f in opts.build_dir.glob('*-uninstalled.pc'):
            # move() does not allow us to update the file so remove it if it already exists
            dest = uninstalled / P(f).name
            if dest.exists():
                dest.unlink()
            # move() takes paths from Python3.9 on
            if (sys.version_info.major >= 3) and (sys.version_info.minor >= 9):
                shutil.move(f, uninstalled)
            else:
                shutil.move(str(f), str(uninstalled))
