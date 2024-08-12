# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_neocmakelsp_global_optspecs
	string join \n h/help V/version
end

function __fish_neocmakelsp_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_neocmakelsp_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_neocmakelsp_using_subcommand
	set -l cmd (__fish_neocmakelsp_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c neocmakelsp -n "__fish_neocmakelsp_needs_command" -s h -l help -d 'Print help'
complete -c neocmakelsp -n "__fish_neocmakelsp_needs_command" -s V -l version -d 'Print version'
complete -c neocmakelsp -n "__fish_neocmakelsp_needs_command" -f -a "stdio" -d 'run with stdio'
complete -c neocmakelsp -n "__fish_neocmakelsp_needs_command" -f -a "tcp" -d 'run with tcp'
complete -c neocmakelsp -n "__fish_neocmakelsp_needs_command" -f -a "search" -d 'search the packages'
complete -c neocmakelsp -n "__fish_neocmakelsp_needs_command" -f -a "format" -d 'Format the file'
complete -c neocmakelsp -n "__fish_neocmakelsp_needs_command" -f -a "tree" -d 'show the file tree'
complete -c neocmakelsp -n "__fish_neocmakelsp_needs_command" -f -a "gen-completions" -d 'genarate the completion'
complete -c neocmakelsp -n "__fish_neocmakelsp_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c neocmakelsp -n "__fish_neocmakelsp_using_subcommand stdio" -s h -l help -d 'Print help'
complete -c neocmakelsp -n "__fish_neocmakelsp_using_subcommand tcp" -l port -r
complete -c neocmakelsp -n "__fish_neocmakelsp_using_subcommand tcp" -s h -l help -d 'Print help'
complete -c neocmakelsp -n "__fish_neocmakelsp_using_subcommand search" -s j
complete -c neocmakelsp -n "__fish_neocmakelsp_using_subcommand search" -s h -l help -d 'Print help'
complete -c neocmakelsp -n "__fish_neocmakelsp_using_subcommand format" -s o -l override
complete -c neocmakelsp -n "__fish_neocmakelsp_using_subcommand format" -s h -l help -d 'Print help'
complete -c neocmakelsp -n "__fish_neocmakelsp_using_subcommand tree" -s j
complete -c neocmakelsp -n "__fish_neocmakelsp_using_subcommand tree" -s h -l help -d 'Print help'
complete -c neocmakelsp -n "__fish_neocmakelsp_using_subcommand gen-completions" -s h -l help -d 'Print help'
complete -c neocmakelsp -n "__fish_neocmakelsp_using_subcommand help; and not __fish_seen_subcommand_from stdio tcp search format tree gen-completions help" -f -a "stdio" -d 'run with stdio'
complete -c neocmakelsp -n "__fish_neocmakelsp_using_subcommand help; and not __fish_seen_subcommand_from stdio tcp search format tree gen-completions help" -f -a "tcp" -d 'run with tcp'
complete -c neocmakelsp -n "__fish_neocmakelsp_using_subcommand help; and not __fish_seen_subcommand_from stdio tcp search format tree gen-completions help" -f -a "search" -d 'search the packages'
complete -c neocmakelsp -n "__fish_neocmakelsp_using_subcommand help; and not __fish_seen_subcommand_from stdio tcp search format tree gen-completions help" -f -a "format" -d 'Format the file'
complete -c neocmakelsp -n "__fish_neocmakelsp_using_subcommand help; and not __fish_seen_subcommand_from stdio tcp search format tree gen-completions help" -f -a "tree" -d 'show the file tree'
complete -c neocmakelsp -n "__fish_neocmakelsp_using_subcommand help; and not __fish_seen_subcommand_from stdio tcp search format tree gen-completions help" -f -a "gen-completions" -d 'genarate the completion'
complete -c neocmakelsp -n "__fish_neocmakelsp_using_subcommand help; and not __fish_seen_subcommand_from stdio tcp search format tree gen-completions help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
