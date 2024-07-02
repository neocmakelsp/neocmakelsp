complete -c neocmakelsp -n "__fish_use_subcommand" -s h -l help -d 'Print help'
complete -c neocmakelsp -n "__fish_use_subcommand" -s V -l version -d 'Print version'
complete -c neocmakelsp -n "__fish_use_subcommand" -f -a "stdio" -d 'run with stdio'
complete -c neocmakelsp -n "__fish_use_subcommand" -f -a "tcp" -d 'run with tcp'
complete -c neocmakelsp -n "__fish_use_subcommand" -f -a "search" -d 'search the packages'
complete -c neocmakelsp -n "__fish_use_subcommand" -f -a "format" -d 'Format the file'
complete -c neocmakelsp -n "__fish_use_subcommand" -f -a "tree" -d 'show the file tree'
complete -c neocmakelsp -n "__fish_use_subcommand" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c neocmakelsp -n "__fish_seen_subcommand_from stdio" -s h -l help -d 'Print help'
complete -c neocmakelsp -n "__fish_seen_subcommand_from tcp" -l port -r
complete -c neocmakelsp -n "__fish_seen_subcommand_from tcp" -s h -l help -d 'Print help'
complete -c neocmakelsp -n "__fish_seen_subcommand_from search" -s j
complete -c neocmakelsp -n "__fish_seen_subcommand_from search" -s h -l help -d 'Print help'
complete -c neocmakelsp -n "__fish_seen_subcommand_from format" -s o -l override
complete -c neocmakelsp -n "__fish_seen_subcommand_from format" -s h -l help -d 'Print help'
complete -c neocmakelsp -n "__fish_seen_subcommand_from tree" -s j
complete -c neocmakelsp -n "__fish_seen_subcommand_from tree" -s h -l help -d 'Print help'
complete -c neocmakelsp -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from stdio tcp search format tree help" -f -a "stdio" -d 'run with stdio'
complete -c neocmakelsp -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from stdio tcp search format tree help" -f -a "tcp" -d 'run with tcp'
complete -c neocmakelsp -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from stdio tcp search format tree help" -f -a "search" -d 'search the packages'
complete -c neocmakelsp -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from stdio tcp search format tree help" -f -a "format" -d 'Format the file'
complete -c neocmakelsp -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from stdio tcp search format tree help" -f -a "tree" -d 'show the file tree'
complete -c neocmakelsp -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from stdio tcp search format tree help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
