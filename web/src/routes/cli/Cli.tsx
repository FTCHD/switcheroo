import { useProviders } from '@/api/queries'
import { PageHeader } from '@/shell/PageHeader'
import { Panel } from '@/shell/Panel'
import { CommandRow } from './CommandRow'

/**
 * Reference for the `switcheroo` command. Examples use the CLIs and account actually present
 * on this machine so they can be pasted as-is.
 */
export function CliPage() {
    const providers = useProviders()
    const installed = (providers.data ?? []).filter((p) => p.installed)
    const first = installed.find((p) => p.live) ?? installed[0]
    const pid = first?.info.id ?? 'claude-code'
    const account = first?.live?.id ?? 'work@example.com'
    const second = installed.find((p) => p.info.id !== pid)?.info.id ?? 'github-cli'

    return (
        <>
            <PageHeader
                title="Command line"
                description="Everything the web UI and tray do is available as switcheroo commands, which makes it easy to script or use from the terminal you are already in. Add --json to any command for machine-readable output."
            />

            <Panel eyebrow="Everyday">
                <CommandRow command="switcheroo [status] [--refresh]">
                    Show every detected CLI, who it is signed in as, and the remembered accounts.
                    Same as <code>switcheroo status</code>. Add <code>--refresh</code> to ask each
                    CLI again instead of using the cached identity.
                </CommandRow>
                <CommandRow
                    command="switcheroo save <provider> [--label NAME]"
                    example={`switcheroo save ${pid} --label work`}
                >
                    Remember the CLI's current login so you can switch back to it later. The label
                    is optional; the email or username is used otherwise.
                </CommandRow>
                <CommandRow
                    command="switcheroo login <provider> [--label NAME]"
                    example={`switcheroo login ${pid}`}
                >
                    Run the CLI's own sign-in in this terminal, then remember the result. This is
                    how you add a second account.
                </CommandRow>
                <CommandRow
                    command="switcheroo use <provider> [account]"
                    example={`switcheroo use ${pid} ${account}`}
                >
                    Switch the live login. The account can be an email, a label, or a unique part of
                    either; leave it out for a picker. The current login is remembered first, so
                    nothing is lost.
                </CommandRow>
            </Panel>

            <Panel eyebrow="Usage">
                <CommandRow
                    command="switcheroo usage [provider] [--refresh]"
                    example={`switcheroo usage ${pid}`}
                >
                    How much of each signed-in account's allowance is used and when it resets, for
                    the CLIs that report it (Claude Code, Codex, GitHub CLI). Answers are cached for
                    a minute; <code>--refresh</code> asks again.
                </CommandRow>
            </Panel>

            <Panel eyebrow="Remembered accounts">
                <CommandRow command="switcheroo list [provider]" example={`switcheroo list ${pid}`}>
                    List remembered accounts, for one CLI or all of them, with which one is active
                    and when it was last used.
                </CommandRow>
                <CommandRow
                    command="switcheroo rename <provider> <account> <label>"
                    example={`switcheroo rename ${pid} ${account} personal`}
                >
                    Give an account a friendlier name. Labels work everywhere an account is named.
                </CommandRow>
                <CommandRow
                    command="switcheroo remove <provider> <account> [--yes]"
                    example={`switcheroo remove ${pid} personal`}
                >
                    Forget an account and delete its stored credential. The CLI itself is not logged
                    out. <code>--yes</code> skips the confirmation.
                </CommandRow>
            </Panel>

            <Panel eyebrow="Diagnostics">
                <CommandRow command="switcheroo doctor" example="switcheroo doctor">
                    Where secrets are stored, which CLIs were found and where, who they are signed
                    in as, and environment variables that would make a switch do nothing. Same as
                    the Doctor page.
                </CommandRow>
                <CommandRow command="switcheroo providers" example="switcheroo providers --json">
                    Every CLI Switcheroo knows about, how each one is switched, and what it touches,
                    including ones that are not installed here.
                </CommandRow>
            </Panel>

            <Panel eyebrow="Running the app">
                <CommandRow command="switcheroo tray" example="switcheroo tray">
                    Start the menu-bar quick switcher together with this web UI. Keep it running;
                    the menu updates when you switch from a terminal too.
                </CommandRow>
                <CommandRow
                    command="switcheroo serve [--bind HOST:PORT]"
                    example="switcheroo serve --open"
                >
                    Serve only the web UI, on loopback. <code>--open</code> opens the browser once
                    it is listening. Useful on machines without a tray.
                </CommandRow>
                <CommandRow
                    command="switcheroo autostart enable | disable | status"
                    example="switcheroo autostart enable"
                >
                    Start the tray automatically when you log in: a LaunchAgent on macOS, the
                    per-user Run key on Windows, an XDG autostart entry on Linux. Nothing is
                    launched right away; run <code>switcheroo tray</code> for that.
                </CommandRow>
                <CommandRow command="switcheroo open" example="switcheroo open">
                    Open the web UI in your browser, starting the server first if nothing is
                    running.
                </CommandRow>
                <CommandRow
                    command="switcheroo completions <shell>"
                    example="switcheroo completions zsh > ~/.zfunc/_switcheroo"
                >
                    Print shell completions for bash, zsh, fish, PowerShell or elvish.
                </CommandRow>
            </Panel>

            <Panel eyebrow="Options that work with every command">
                <CommandRow
                    command="--json"
                    example={`switcheroo status --json | jq '.[] | select(.live)'`}
                >
                    Print the same structures the web UI uses instead of a table.
                </CommandRow>
                <CommandRow
                    command="--vault auto | keychain | file"
                    example="switcheroo --vault file doctor"
                >
                    Where remembered credentials go: the OS credential store, or a plain file with
                    0600 permissions for machines without one. Also set by{' '}
                    <code>SWITCHEROO_VAULT</code>.
                </CommandRow>
                <CommandRow
                    command="--data-dir DIR"
                    example="switcheroo --data-dir ~/.config/switcheroo-test status"
                >
                    Keep state somewhere else, for example to try things without touching your real
                    setup. Also set by <code>SWITCHEROO_DATA_DIR</code>.
                </CommandRow>
                <CommandRow command="-v, -q" example={`switcheroo -v use ${second} `.trimEnd()}>
                    More log output (repeat for more), or errors only.
                </CommandRow>
            </Panel>

            <Panel eyebrow="Exit codes">
                <CommandRow command="0 · 1 · 2">
                    Success, an error that was printed, or a usage mistake.
                </CommandRow>
                <CommandRow command="3">The CLI you named is not installed on PATH.</CommandRow>
                <CommandRow command="4">
                    Nothing is signed in, so there was no login to remember.
                </CommandRow>
            </Panel>
        </>
    )
}
