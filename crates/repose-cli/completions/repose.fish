# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_repose_global_optspecs
    string join \n n/print V/version c/config= d/debug q/quiet no-color format= strict-host-key-checking= known-hosts= h/help
end

function __fish_repose_needs_command
    # Figure out if the current invocation already has a command.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    argparse -s (__fish_repose_global_optspecs) -- $cmd 2>/dev/null
    or return
    if set -q argv[1]
        # Also print the command, so this can be used to figure out what it is.
        echo $argv[1]
        return 1
    end
    return 0
end

function __fish_repose_using_subcommand
    set -l cmd (__fish_repose_needs_command)
    test -z "$cmd"
    and return 1
    contains -- $cmd[1] $argv
end

complete -c repose -n "__fish_repose_needs_command" -s c -l config -d 'path to repose configuration' -r -F
complete -c repose -n "__fish_repose_needs_command" -l format -d 'console output format: \'text\' (default) or \'json\' (one event per line)' -r -f -a "text\t''
json\t''"
complete -c repose -n "__fish_repose_needs_command" -l strict-host-key-checking -d 'SSH host-key policy (OpenSSH semantics): \'yes\' refuses unknown hosts; \'accept-new\' (default) accepts unknown hosts on first contact but rejects changed keys; \'no\'/\'off\' accepts both unknown and changed keys (pre-1.12 behaviour)' -r -f -a "yes\t''
accept-new\t''
no\t''
off\t''"
complete -c repose -n "__fish_repose_needs_command" -l known-hosts -d 'path to a custom known_hosts file (overrides ~/.ssh/known_hosts)' -r -F
complete -c repose -n "__fish_repose_needs_command" -s n -l print -d 'print commands for host and exit'
complete -c repose -n "__fish_repose_needs_command" -s V -l version -d 'show program\'s version number and exit'
complete -c repose -n "__fish_repose_needs_command" -s d -l debug -d 'enable debug logging'
complete -c repose -n "__fish_repose_needs_command" -s q -l quiet -d 'suppress messages from repose'
complete -c repose -n "__fish_repose_needs_command" -l no-color -d 'disable ANSI color in console output (honors NO_COLOR)'
complete -c repose -n "__fish_repose_needs_command" -s h -l help -d 'Print help'
complete -c repose -n "__fish_repose_needs_command" -f -a "add" -d 'add specified repository to target'
complete -c repose -n "__fish_repose_needs_command" -f -a "remove" -d 'remove repository from target'
complete -c repose -n "__fish_repose_needs_command" -f -a "reset" -d 'reset target repositories to only installed products repositories'
complete -c repose -n "__fish_repose_needs_command" -f -a "install" -d 'add specified repository to target and install product'
complete -c repose -n "__fish_repose_needs_command" -f -a "clear" -d 'clear all repositories from target'
complete -c repose -n "__fish_repose_needs_command" -f -a "uninstall" -d 'remove specified repository from target and uninstall product'
complete -c repose -n "__fish_repose_needs_command" -f -a "list-products" -d 'list products on target'
complete -c repose -n "__fish_repose_needs_command" -f -a "list-repos" -d 'list repositories on target'
complete -c repose -n "__fish_repose_needs_command" -f -a "known-products" -d 'list known products by \'repose\''
complete -c repose -n "__fish_repose_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c repose -n "__fish_repose_using_subcommand add" -s t -l target -d 'target to operate on' -r
complete -c repose -n "__fish_repose_using_subcommand add" -l probe-timeout -d 'seconds to wait per repository URL probe (default: 5)' -r
complete -c repose -n "__fish_repose_using_subcommand add" -s c -l config -d 'path to repose configuration' -r -F
complete -c repose -n "__fish_repose_using_subcommand add" -l format -d 'console output format: \'text\' (default) or \'json\' (one event per line)' -r -f -a "text\t''
json\t''"
complete -c repose -n "__fish_repose_using_subcommand add" -l strict-host-key-checking -d 'SSH host-key policy (OpenSSH semantics): \'yes\' refuses unknown hosts; \'accept-new\' (default) accepts unknown hosts on first contact but rejects changed keys; \'no\'/\'off\' accepts both unknown and changed keys (pre-1.12 behaviour)' -r -f -a "yes\t''
accept-new\t''
no\t''
off\t''"
complete -c repose -n "__fish_repose_using_subcommand add" -l known-hosts -d 'path to a custom known_hosts file (overrides ~/.ssh/known_hosts)' -r -F
complete -c repose -n "__fish_repose_using_subcommand add" -l no-probe -d 'skip repository URL liveness probes'
complete -c repose -n "__fish_repose_using_subcommand add" -s n -l print -d 'print commands for host and exit'
complete -c repose -n "__fish_repose_using_subcommand add" -s V -l version -d 'show program\'s version number and exit'
complete -c repose -n "__fish_repose_using_subcommand add" -s d -l debug -d 'enable debug logging'
complete -c repose -n "__fish_repose_using_subcommand add" -s q -l quiet -d 'suppress messages from repose'
complete -c repose -n "__fish_repose_using_subcommand add" -l no-color -d 'disable ANSI color in console output (honors NO_COLOR)'
complete -c repose -n "__fish_repose_using_subcommand add" -s h -l help -d 'Print help'
complete -c repose -n "__fish_repose_using_subcommand remove" -s t -l target -d 'target to operate on' -r
complete -c repose -n "__fish_repose_using_subcommand remove" -s c -l config -d 'path to repose configuration' -r -F
complete -c repose -n "__fish_repose_using_subcommand remove" -l format -d 'console output format: \'text\' (default) or \'json\' (one event per line)' -r -f -a "text\t''
json\t''"
complete -c repose -n "__fish_repose_using_subcommand remove" -l strict-host-key-checking -d 'SSH host-key policy (OpenSSH semantics): \'yes\' refuses unknown hosts; \'accept-new\' (default) accepts unknown hosts on first contact but rejects changed keys; \'no\'/\'off\' accepts both unknown and changed keys (pre-1.12 behaviour)' -r -f -a "yes\t''
accept-new\t''
no\t''
off\t''"
complete -c repose -n "__fish_repose_using_subcommand remove" -l known-hosts -d 'path to a custom known_hosts file (overrides ~/.ssh/known_hosts)' -r -F
complete -c repose -n "__fish_repose_using_subcommand remove" -s n -l print -d 'print commands for host and exit'
complete -c repose -n "__fish_repose_using_subcommand remove" -s V -l version -d 'show program\'s version number and exit'
complete -c repose -n "__fish_repose_using_subcommand remove" -s d -l debug -d 'enable debug logging'
complete -c repose -n "__fish_repose_using_subcommand remove" -s q -l quiet -d 'suppress messages from repose'
complete -c repose -n "__fish_repose_using_subcommand remove" -l no-color -d 'disable ANSI color in console output (honors NO_COLOR)'
complete -c repose -n "__fish_repose_using_subcommand remove" -s h -l help -d 'Print help'
complete -c repose -n "__fish_repose_using_subcommand reset" -s t -l target -d 'target to operate on' -r
complete -c repose -n "__fish_repose_using_subcommand reset" -l probe-timeout -d 'seconds to wait per repository URL probe (default: 5)' -r
complete -c repose -n "__fish_repose_using_subcommand reset" -s c -l config -d 'path to repose configuration' -r -F
complete -c repose -n "__fish_repose_using_subcommand reset" -l format -d 'console output format: \'text\' (default) or \'json\' (one event per line)' -r -f -a "text\t''
json\t''"
complete -c repose -n "__fish_repose_using_subcommand reset" -l strict-host-key-checking -d 'SSH host-key policy (OpenSSH semantics): \'yes\' refuses unknown hosts; \'accept-new\' (default) accepts unknown hosts on first contact but rejects changed keys; \'no\'/\'off\' accepts both unknown and changed keys (pre-1.12 behaviour)' -r -f -a "yes\t''
accept-new\t''
no\t''
off\t''"
complete -c repose -n "__fish_repose_using_subcommand reset" -l known-hosts -d 'path to a custom known_hosts file (overrides ~/.ssh/known_hosts)' -r -F
complete -c repose -n "__fish_repose_using_subcommand reset" -l no-probe -d 'skip repository URL liveness probes'
complete -c repose -n "__fish_repose_using_subcommand reset" -s n -l print -d 'print commands for host and exit'
complete -c repose -n "__fish_repose_using_subcommand reset" -s V -l version -d 'show program\'s version number and exit'
complete -c repose -n "__fish_repose_using_subcommand reset" -s d -l debug -d 'enable debug logging'
complete -c repose -n "__fish_repose_using_subcommand reset" -s q -l quiet -d 'suppress messages from repose'
complete -c repose -n "__fish_repose_using_subcommand reset" -l no-color -d 'disable ANSI color in console output (honors NO_COLOR)'
complete -c repose -n "__fish_repose_using_subcommand reset" -s h -l help -d 'Print help'
complete -c repose -n "__fish_repose_using_subcommand install" -s t -l target -d 'target to operate on' -r
complete -c repose -n "__fish_repose_using_subcommand install" -l probe-timeout -d 'seconds to wait per repository URL probe (default: 5)' -r
complete -c repose -n "__fish_repose_using_subcommand install" -s c -l config -d 'path to repose configuration' -r -F
complete -c repose -n "__fish_repose_using_subcommand install" -l format -d 'console output format: \'text\' (default) or \'json\' (one event per line)' -r -f -a "text\t''
json\t''"
complete -c repose -n "__fish_repose_using_subcommand install" -l strict-host-key-checking -d 'SSH host-key policy (OpenSSH semantics): \'yes\' refuses unknown hosts; \'accept-new\' (default) accepts unknown hosts on first contact but rejects changed keys; \'no\'/\'off\' accepts both unknown and changed keys (pre-1.12 behaviour)' -r -f -a "yes\t''
accept-new\t''
no\t''
off\t''"
complete -c repose -n "__fish_repose_using_subcommand install" -l known-hosts -d 'path to a custom known_hosts file (overrides ~/.ssh/known_hosts)' -r -F
complete -c repose -n "__fish_repose_using_subcommand install" -l no-probe -d 'skip repository URL liveness probes'
complete -c repose -n "__fish_repose_using_subcommand install" -l no-reboot -d 'on transactional hosts (SL Micro), stage the package change but do not reboot/reconnect/verify (default: reboot)'
complete -c repose -n "__fish_repose_using_subcommand install" -s n -l print -d 'print commands for host and exit'
complete -c repose -n "__fish_repose_using_subcommand install" -s V -l version -d 'show program\'s version number and exit'
complete -c repose -n "__fish_repose_using_subcommand install" -s d -l debug -d 'enable debug logging'
complete -c repose -n "__fish_repose_using_subcommand install" -s q -l quiet -d 'suppress messages from repose'
complete -c repose -n "__fish_repose_using_subcommand install" -l no-color -d 'disable ANSI color in console output (honors NO_COLOR)'
complete -c repose -n "__fish_repose_using_subcommand install" -s h -l help -d 'Print help'
complete -c repose -n "__fish_repose_using_subcommand clear" -s t -l target -d 'target to operate on' -r
complete -c repose -n "__fish_repose_using_subcommand clear" -s c -l config -d 'path to repose configuration' -r -F
complete -c repose -n "__fish_repose_using_subcommand clear" -l format -d 'console output format: \'text\' (default) or \'json\' (one event per line)' -r -f -a "text\t''
json\t''"
complete -c repose -n "__fish_repose_using_subcommand clear" -l strict-host-key-checking -d 'SSH host-key policy (OpenSSH semantics): \'yes\' refuses unknown hosts; \'accept-new\' (default) accepts unknown hosts on first contact but rejects changed keys; \'no\'/\'off\' accepts both unknown and changed keys (pre-1.12 behaviour)' -r -f -a "yes\t''
accept-new\t''
no\t''
off\t''"
complete -c repose -n "__fish_repose_using_subcommand clear" -l known-hosts -d 'path to a custom known_hosts file (overrides ~/.ssh/known_hosts)' -r -F
complete -c repose -n "__fish_repose_using_subcommand clear" -s n -l print -d 'print commands for host and exit'
complete -c repose -n "__fish_repose_using_subcommand clear" -s V -l version -d 'show program\'s version number and exit'
complete -c repose -n "__fish_repose_using_subcommand clear" -s d -l debug -d 'enable debug logging'
complete -c repose -n "__fish_repose_using_subcommand clear" -s q -l quiet -d 'suppress messages from repose'
complete -c repose -n "__fish_repose_using_subcommand clear" -l no-color -d 'disable ANSI color in console output (honors NO_COLOR)'
complete -c repose -n "__fish_repose_using_subcommand clear" -s h -l help -d 'Print help'
complete -c repose -n "__fish_repose_using_subcommand uninstall" -s t -l target -d 'target to operate on' -r
complete -c repose -n "__fish_repose_using_subcommand uninstall" -s c -l config -d 'path to repose configuration' -r -F
complete -c repose -n "__fish_repose_using_subcommand uninstall" -l format -d 'console output format: \'text\' (default) or \'json\' (one event per line)' -r -f -a "text\t''
json\t''"
complete -c repose -n "__fish_repose_using_subcommand uninstall" -l strict-host-key-checking -d 'SSH host-key policy (OpenSSH semantics): \'yes\' refuses unknown hosts; \'accept-new\' (default) accepts unknown hosts on first contact but rejects changed keys; \'no\'/\'off\' accepts both unknown and changed keys (pre-1.12 behaviour)' -r -f -a "yes\t''
accept-new\t''
no\t''
off\t''"
complete -c repose -n "__fish_repose_using_subcommand uninstall" -l known-hosts -d 'path to a custom known_hosts file (overrides ~/.ssh/known_hosts)' -r -F
complete -c repose -n "__fish_repose_using_subcommand uninstall" -l no-reboot -d 'on transactional hosts (SL Micro), stage the package change but do not reboot/reconnect/verify (default: reboot)'
complete -c repose -n "__fish_repose_using_subcommand uninstall" -s n -l print -d 'print commands for host and exit'
complete -c repose -n "__fish_repose_using_subcommand uninstall" -s V -l version -d 'show program\'s version number and exit'
complete -c repose -n "__fish_repose_using_subcommand uninstall" -s d -l debug -d 'enable debug logging'
complete -c repose -n "__fish_repose_using_subcommand uninstall" -s q -l quiet -d 'suppress messages from repose'
complete -c repose -n "__fish_repose_using_subcommand uninstall" -l no-color -d 'disable ANSI color in console output (honors NO_COLOR)'
complete -c repose -n "__fish_repose_using_subcommand uninstall" -s h -l help -d 'Print help'
complete -c repose -n "__fish_repose_using_subcommand list-products" -s t -l target -d 'target to operate on' -r
complete -c repose -n "__fish_repose_using_subcommand list-products" -s c -l config -d 'path to repose configuration' -r -F
complete -c repose -n "__fish_repose_using_subcommand list-products" -l format -d 'console output format: \'text\' (default) or \'json\' (one event per line)' -r -f -a "text\t''
json\t''"
complete -c repose -n "__fish_repose_using_subcommand list-products" -l strict-host-key-checking -d 'SSH host-key policy (OpenSSH semantics): \'yes\' refuses unknown hosts; \'accept-new\' (default) accepts unknown hosts on first contact but rejects changed keys; \'no\'/\'off\' accepts both unknown and changed keys (pre-1.12 behaviour)' -r -f -a "yes\t''
accept-new\t''
no\t''
off\t''"
complete -c repose -n "__fish_repose_using_subcommand list-products" -l known-hosts -d 'path to a custom known_hosts file (overrides ~/.ssh/known_hosts)' -r -F
complete -c repose -n "__fish_repose_using_subcommand list-products" -l yaml -d 'Generate YAML host spec for refhosts.yml generator without normalization. Default for SLE 12-SP5 and SLE 15-SP3+ products'
complete -c repose -n "__fish_repose_using_subcommand list-products" -s n -l print -d 'print commands for host and exit'
complete -c repose -n "__fish_repose_using_subcommand list-products" -s V -l version -d 'show program\'s version number and exit'
complete -c repose -n "__fish_repose_using_subcommand list-products" -s d -l debug -d 'enable debug logging'
complete -c repose -n "__fish_repose_using_subcommand list-products" -s q -l quiet -d 'suppress messages from repose'
complete -c repose -n "__fish_repose_using_subcommand list-products" -l no-color -d 'disable ANSI color in console output (honors NO_COLOR)'
complete -c repose -n "__fish_repose_using_subcommand list-products" -s h -l help -d 'Print help'
complete -c repose -n "__fish_repose_using_subcommand list-repos" -s t -l target -d 'target to operate on' -r
complete -c repose -n "__fish_repose_using_subcommand list-repos" -s c -l config -d 'path to repose configuration' -r -F
complete -c repose -n "__fish_repose_using_subcommand list-repos" -l format -d 'console output format: \'text\' (default) or \'json\' (one event per line)' -r -f -a "text\t''
json\t''"
complete -c repose -n "__fish_repose_using_subcommand list-repos" -l strict-host-key-checking -d 'SSH host-key policy (OpenSSH semantics): \'yes\' refuses unknown hosts; \'accept-new\' (default) accepts unknown hosts on first contact but rejects changed keys; \'no\'/\'off\' accepts both unknown and changed keys (pre-1.12 behaviour)' -r -f -a "yes\t''
accept-new\t''
no\t''
off\t''"
complete -c repose -n "__fish_repose_using_subcommand list-repos" -l known-hosts -d 'path to a custom known_hosts file (overrides ~/.ssh/known_hosts)' -r -F
complete -c repose -n "__fish_repose_using_subcommand list-repos" -s n -l print -d 'print commands for host and exit'
complete -c repose -n "__fish_repose_using_subcommand list-repos" -s V -l version -d 'show program\'s version number and exit'
complete -c repose -n "__fish_repose_using_subcommand list-repos" -s d -l debug -d 'enable debug logging'
complete -c repose -n "__fish_repose_using_subcommand list-repos" -s q -l quiet -d 'suppress messages from repose'
complete -c repose -n "__fish_repose_using_subcommand list-repos" -l no-color -d 'disable ANSI color in console output (honors NO_COLOR)'
complete -c repose -n "__fish_repose_using_subcommand list-repos" -s h -l help -d 'Print help'
complete -c repose -n "__fish_repose_using_subcommand known-products" -s c -l config -d 'path to repose configuration' -r -F
complete -c repose -n "__fish_repose_using_subcommand known-products" -l format -d 'console output format: \'text\' (default) or \'json\' (one event per line)' -r -f -a "text\t''
json\t''"
complete -c repose -n "__fish_repose_using_subcommand known-products" -l strict-host-key-checking -d 'SSH host-key policy (OpenSSH semantics): \'yes\' refuses unknown hosts; \'accept-new\' (default) accepts unknown hosts on first contact but rejects changed keys; \'no\'/\'off\' accepts both unknown and changed keys (pre-1.12 behaviour)' -r -f -a "yes\t''
accept-new\t''
no\t''
off\t''"
complete -c repose -n "__fish_repose_using_subcommand known-products" -l known-hosts -d 'path to a custom known_hosts file (overrides ~/.ssh/known_hosts)' -r -F
complete -c repose -n "__fish_repose_using_subcommand known-products" -s n -l print -d 'print commands for host and exit'
complete -c repose -n "__fish_repose_using_subcommand known-products" -s V -l version -d 'show program\'s version number and exit'
complete -c repose -n "__fish_repose_using_subcommand known-products" -s d -l debug -d 'enable debug logging'
complete -c repose -n "__fish_repose_using_subcommand known-products" -s q -l quiet -d 'suppress messages from repose'
complete -c repose -n "__fish_repose_using_subcommand known-products" -l no-color -d 'disable ANSI color in console output (honors NO_COLOR)'
complete -c repose -n "__fish_repose_using_subcommand known-products" -s h -l help -d 'Print help'
complete -c repose -n "__fish_repose_using_subcommand help; and not __fish_seen_subcommand_from add remove reset install clear uninstall list-products list-repos known-products help" -f -a "add" -d 'add specified repository to target'
complete -c repose -n "__fish_repose_using_subcommand help; and not __fish_seen_subcommand_from add remove reset install clear uninstall list-products list-repos known-products help" -f -a "remove" -d 'remove repository from target'
complete -c repose -n "__fish_repose_using_subcommand help; and not __fish_seen_subcommand_from add remove reset install clear uninstall list-products list-repos known-products help" -f -a "reset" -d 'reset target repositories to only installed products repositories'
complete -c repose -n "__fish_repose_using_subcommand help; and not __fish_seen_subcommand_from add remove reset install clear uninstall list-products list-repos known-products help" -f -a "install" -d 'add specified repository to target and install product'
complete -c repose -n "__fish_repose_using_subcommand help; and not __fish_seen_subcommand_from add remove reset install clear uninstall list-products list-repos known-products help" -f -a "clear" -d 'clear all repositories from target'
complete -c repose -n "__fish_repose_using_subcommand help; and not __fish_seen_subcommand_from add remove reset install clear uninstall list-products list-repos known-products help" -f -a "uninstall" -d 'remove specified repository from target and uninstall product'
complete -c repose -n "__fish_repose_using_subcommand help; and not __fish_seen_subcommand_from add remove reset install clear uninstall list-products list-repos known-products help" -f -a "list-products" -d 'list products on target'
complete -c repose -n "__fish_repose_using_subcommand help; and not __fish_seen_subcommand_from add remove reset install clear uninstall list-products list-repos known-products help" -f -a "list-repos" -d 'list repositories on target'
complete -c repose -n "__fish_repose_using_subcommand help; and not __fish_seen_subcommand_from add remove reset install clear uninstall list-products list-repos known-products help" -f -a "known-products" -d 'list known products by \'repose\''
complete -c repose -n "__fish_repose_using_subcommand help; and not __fish_seen_subcommand_from add remove reset install clear uninstall list-products list-repos known-products help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
