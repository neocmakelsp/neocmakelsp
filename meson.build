project(
  'neocmakelsp',
  'rust',
  version: '0.8.2',
  meson_version : '>= 1.1.0'
)

cargo = find_program('cargo', version: '>= 1.80')

find_program('rustc', version: '>= 1.80')

if get_option('debug')
  command = [
    cargo, 'build',
    '&&',
    'cp', meson.global_source_root() / 'target' / 'debug' / meson.project_name(), '@OUTPUT@'
  ]
else
  command = [
    cargo, 'build',
    '--release',
    '&&',
    'cp', meson.global_source_root() / 'target' / 'release' / meson.project_name(), '@OUTPUT@'
  ]
endif

prefix = get_option('prefix')

bindir = prefix / get_option('bindir')
datadir = prefix / get_option('datadir')

fish_compeletion_dir = datadir / 'fish' / 'functions'
bash_compeletion_dir = datadir / 'bash-compeletion' / 'completions'
zsh_compeletion_dir = datadir / 'zsh' / 'site-functions'

custom_target('neocmakelsp',
  output: 'neocmakelsp',
  build_by_default: true,
  install: true,
  install_dir: bindir,
  console: true,
  command: command
)

install_data('completions/zsh/_neocmakelsp',
  install_dir: zsh_compeletion_dir
)

install_data('completions/fish/neocmakelsp.fish',
  install_dir: fish_compeletion_dir
)

install_data('completions/bash/neocmakelsp',
  install_dir: bash_compeletion_dir
)
