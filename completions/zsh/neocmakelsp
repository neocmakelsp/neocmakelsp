#compdef neocmakelsp

autoload -U is-at-least

_neocmakelsp() {
    typeset -A opt_args
    typeset -a _arguments_options
    local ret=1

    if is-at-least 5.2; then
        _arguments_options=(-s -S -C)
    else
        _arguments_options=(-s -C)
    fi

    local context curcontext="$curcontext" state line
    _arguments "${_arguments_options[@]}" : \
'-h[Print help]' \
'--help[Print help]' \
":: :_neocmakelsp_commands" \
"*::: :->neocmakelsp" \
&& ret=0
    case $state in
    (neocmakelsp)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:neocmakelsp-command-$line[1]:"
        case $line[1] in
            (stdio)
_arguments "${_arguments_options[@]}" : \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(tcp)
_arguments "${_arguments_options[@]}" : \
'-P+[listen to port]: : ' \
'--port=[listen to port]: : ' \
'-h[Print help]' \
'--help[Print help]' \
&& ret=0
;;
(search)
_arguments "${_arguments_options[@]}" : \
'-j[tojson]' \
'--tojson[tojson]' \
'-h[Print help]' \
'--help[Print help]' \
'*::Package -- Packages:' \
&& ret=0
;;
(format)
_arguments "${_arguments_options[@]}" : \
'-o[override]' \
'--override[override]' \
'-h[Print help]' \
'--help[Print help]' \
'*::FormatPath -- file or folder to format:' \
&& ret=0
;;
(tree)
_arguments "${_arguments_options[@]}" : \
'-j[tojson]' \
'--tojson[tojson]' \
'-h[Print help]' \
'--help[Print help]' \
'*::PATH -- tree:_files' \
&& ret=0
;;
(complete)
_arguments "${_arguments_options[@]}" : \
'--shell=[Specify shell to complete for]:SHELL:(bash fish)' \
'--register=[Path to write completion-registration to]:REGISTER:_files' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'*::comp_words:' \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
":: :_neocmakelsp__help_commands" \
"*::: :->help" \
&& ret=0

    case $state in
    (help)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:neocmakelsp-help-command-$line[1]:"
        case $line[1] in
            (stdio)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(tcp)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(search)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(format)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(tree)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(complete)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
        esac
    ;;
esac
;;
        esac
    ;;
esac
}

(( $+functions[_neocmakelsp_commands] )) ||
_neocmakelsp_commands() {
    local commands; commands=(
'stdio:run with stdio' \
'tcp:run with tcp' \
'search:Search packages' \
'format:format the file' \
'tree:Tree the file' \
'complete:Register shell completions for this program' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'neocmakelsp commands' commands "$@"
}
(( $+functions[_neocmakelsp__complete_commands] )) ||
_neocmakelsp__complete_commands() {
    local commands; commands=()
    _describe -t commands 'neocmakelsp complete commands' commands "$@"
}
(( $+functions[_neocmakelsp__format_commands] )) ||
_neocmakelsp__format_commands() {
    local commands; commands=()
    _describe -t commands 'neocmakelsp format commands' commands "$@"
}
(( $+functions[_neocmakelsp__help_commands] )) ||
_neocmakelsp__help_commands() {
    local commands; commands=(
'stdio:run with stdio' \
'tcp:run with tcp' \
'search:Search packages' \
'format:format the file' \
'tree:Tree the file' \
'complete:Register shell completions for this program' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'neocmakelsp help commands' commands "$@"
}
(( $+functions[_neocmakelsp__help__complete_commands] )) ||
_neocmakelsp__help__complete_commands() {
    local commands; commands=()
    _describe -t commands 'neocmakelsp help complete commands' commands "$@"
}
(( $+functions[_neocmakelsp__help__format_commands] )) ||
_neocmakelsp__help__format_commands() {
    local commands; commands=()
    _describe -t commands 'neocmakelsp help format commands' commands "$@"
}
(( $+functions[_neocmakelsp__help__help_commands] )) ||
_neocmakelsp__help__help_commands() {
    local commands; commands=()
    _describe -t commands 'neocmakelsp help help commands' commands "$@"
}
(( $+functions[_neocmakelsp__help__search_commands] )) ||
_neocmakelsp__help__search_commands() {
    local commands; commands=()
    _describe -t commands 'neocmakelsp help search commands' commands "$@"
}
(( $+functions[_neocmakelsp__help__stdio_commands] )) ||
_neocmakelsp__help__stdio_commands() {
    local commands; commands=()
    _describe -t commands 'neocmakelsp help stdio commands' commands "$@"
}
(( $+functions[_neocmakelsp__help__tcp_commands] )) ||
_neocmakelsp__help__tcp_commands() {
    local commands; commands=()
    _describe -t commands 'neocmakelsp help tcp commands' commands "$@"
}
(( $+functions[_neocmakelsp__help__tree_commands] )) ||
_neocmakelsp__help__tree_commands() {
    local commands; commands=()
    _describe -t commands 'neocmakelsp help tree commands' commands "$@"
}
(( $+functions[_neocmakelsp__search_commands] )) ||
_neocmakelsp__search_commands() {
    local commands; commands=()
    _describe -t commands 'neocmakelsp search commands' commands "$@"
}
(( $+functions[_neocmakelsp__stdio_commands] )) ||
_neocmakelsp__stdio_commands() {
    local commands; commands=()
    _describe -t commands 'neocmakelsp stdio commands' commands "$@"
}
(( $+functions[_neocmakelsp__tcp_commands] )) ||
_neocmakelsp__tcp_commands() {
    local commands; commands=()
    _describe -t commands 'neocmakelsp tcp commands' commands "$@"
}
(( $+functions[_neocmakelsp__tree_commands] )) ||
_neocmakelsp__tree_commands() {
    local commands; commands=()
    _describe -t commands 'neocmakelsp tree commands' commands "$@"
}

if [ "$funcstack[1]" = "_neocmakelsp" ]; then
    _neocmakelsp "$@"
else
    compdef _neocmakelsp neocmakelsp
fi
