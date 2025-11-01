project('neocmakelsp', 'rust', version: '0.8.23', meson_version: '>= 1.1.0')

find_program('cargo', version: '>= 1.80')

find_program('rustc', version: '>= 1.80')

python = find_program('python')

if get_option('debug')
  command = [
    python,
    meson.global_source_root() / 'cargo_wrapper.py',
    meson.global_source_root(),
    'debug',
    meson.global_source_root() / 'target' / 'debug' / meson.project_name(),
    meson.global_build_root() / '@OUTPUT@',
  ]
else
  command = [
    python,
    meson.global_source_root() / 'cargo_wrapper.py',
    meson.global_source_root(),
    'release',
    meson.global_source_root() / 'target' / 'release' / meson.project_name(),
    meson.global_build_root() / '@OUTPUT@',
  ]
endif

prefix = get_option('prefix')

bindir = prefix / get_option('bindir')
datadir = prefix / get_option('datadir')

fish_completion_dir = datadir / 'fish' / 'vendor_completions.d'
bash_completion_dir = datadir / 'bash-completion' / 'completions'
zsh_completion_dir = datadir / 'zsh' / 'site-functions'

custom_target(
  'neocmakelsp',
  output: 'neocmakelsp',
  build_by_default: true,
  install: true,
  install_dir: bindir,
  console: true,
  command: command,
)

install_data('completions/zsh/_neocmakelsp', install_dir: zsh_completion_dir)

install_data('completions/fish/neocmakelsp.fish', install_dir: fish_completion_dir)

install_data('completions/bash/neocmakelsp', install_dir: bash_completion_dir)
