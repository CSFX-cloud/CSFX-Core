<script lang="ts">
    import CodeInput from "$lib/components/auth/code-input.svelte";
    import Spinner from "$lib/components/ui/spinner/spinner.svelte";
    import { ChevronRight, Download } from "@lucide/svelte";
    import { REGEXP_ONLY_DIGITS_AND_CHARS } from "bits-ui";
    import { goto } from "$app/navigation";
    import { onMount } from "svelte";
    import { toast } from "svelte-sonner";
    import {
        completeSetup,
        getSetupStatus,
        requestClusterJoin,
        SETUP_CA_URL,
        SetupError,
        type SetupStatus,
    } from "$lib/auth/api";
    import { auth } from "$lib/auth/store.svelte";

    type Step = "verify" | "account" | "console" | "join" | "joining" | "done";

    const MIN_PASSWORD_LENGTH = 12;
    const MIN_PIN_LENGTH = 8;
    const SETUP_CODE_LENGTH = 8;
    const FINGERPRINT_OCTETS_PER_LINE = 16;
    const BAR_COUNT = 3;
    const BARS_BY_STEP: Record<Step, number> = {
        verify: 1,
        join: 1,
        joining: 1,
        account: 2,
        console: 3,
        done: 3,
    };

    let step = $state<Step>("verify");
    let status = $state<SetupStatus | null>(null);
    let loadFailed = $state(false);
    let submitting = $state(false);
    let setupCode = $state("");
    let joinString = $state("");
    let organizationName = $state("");
    let adminEmail = $state("");
    let password = $state("");
    let passwordRepeat = $state("");
    let consolePin = $state("");
    let consolePinRepeat = $state("");
    let recoveryKey = $state("");
    let keyStored = $state(false);

    const activeBars = $derived(BARS_BY_STEP[step]);
    const fingerprintLines = $derived(formatFingerprint(status?.ca_fingerprint ?? null));
    const codeValid = $derived(setupCode.length === SETUP_CODE_LENGTH);
    const passwordLongEnough = $derived(password.length >= MIN_PASSWORD_LENGTH);
    const passwordMismatch = $derived(passwordRepeat.length > 0 && password !== passwordRepeat);
    const accountValid = $derived(
        organizationName.trim().length > 0 &&
            /^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(adminEmail.trim()) &&
            passwordLongEnough &&
            password === passwordRepeat,
    );
    const joinValid = $derived(/^csfx-join\.\S+#csfx-bootstrap\.[a-f0-9]+#[a-f0-9]+(#(worker|master))?$/.test(joinString.trim()));
    const pinMismatch = $derived(consolePinRepeat.length > 0 && consolePin !== consolePinRepeat);
    const pinValid = $derived(
        consolePin.length >= MIN_PIN_LENGTH && consolePin === consolePinRepeat,
    );

    onMount(loadStatus);

    function formatFingerprint(fingerprint: string | null) {
        if (!fingerprint) return ["unavailable"];
        const octets = fingerprint.split(":");
        const lines: string[] = [];
        for (let start = 0; start < octets.length; start += FINGERPRINT_OCTETS_PER_LINE) {
            lines.push(octets.slice(start, start + FINGERPRINT_OCTETS_PER_LINE).join(":"));
        }
        return lines;
    }

    async function loadStatus() {
        loadFailed = false;
        try {
            status = await getSetupStatus();
            if (status.state === "ready") goto("/login");
        } catch {
            loadFailed = true;
            setTimeout(loadStatus, 5000);
        }
    }

    function advance() {
        if (step === "verify" && codeValid) step = "account";
        else if (step === "account" && accountValid) step = "console";
        else if (step === "console" && pinValid && !submitting) submit();
        else if (step === "join" && joinValid && !submitting) submitJoin();
    }

    async function submit() {
        submitting = true;
        try {
            const result = await completeSetup({
                setupCode: setupCode.trim(),
                organizationName: organizationName.trim(),
                adminEmail: adminEmail.trim(),
                password,
                consolePin,
            });
            auth.setSession(result);
            recoveryKey = result.recovery_key;
            password = passwordRepeat = consolePin = consolePinRepeat = "";
            step = "done";
        } catch (err) {
            handleFailure(err);
        } finally {
            submitting = false;
        }
    }

    async function submitJoin() {
        submitting = true;
        try {
            await requestClusterJoin(setupCode.trim(), joinString.trim());
            step = "joining";
        } catch (err) {
            handleFailure(err);
        } finally {
            submitting = false;
        }
    }

    function handleFailure(err: unknown) {
        if (!(err instanceof SetupError)) {
            toast.error("Setup failed", { description: "The control plane did not respond" });
            return;
        }
        if (err.status === 410) {
            toast.info("Setup was already completed");
            goto("/login");
            return;
        }
        if (err.status === 401) step = "verify";
        toast.error("Setup failed", { description: err.message });
    }

    async function copyRecoveryKey() {
        await navigator.clipboard.writeText(recoveryKey);
        toast.success("Recovery key copied");
    }
</script>

{#if loadFailed}
    <div class="su-pane">
        <h1 class="cx-headline display-md">Control plane is starting.</h1>
        <p class="su-status">
            <Spinner /> This can take a few minutes after the first boot. Retrying automatically.
        </p>
    </div>
{:else if !status}
    <div class="su-status"><Spinner /></div>
{:else}
    <ol class="su-bars" aria-label="Step {activeBars} of {BAR_COUNT}">
        {#each { length: BAR_COUNT } as _, index (index)}
            <li class="su-bar {index < activeBars ? 'su-bar--on' : ''}"></li>
        {/each}
    </ol>

    {#if step === "verify"}
        <form
            class="su-pane"
            onsubmit={(event) => {
                event.preventDefault();
                advance();
            }}
        >
            <div style="display: grid; gap: 12px">
                <h1 class="cx-headline display-md">Verify this node.</h1>
                <p style="margin: 0; font-size: 15px; line-height: 1.6; color: var(--ink-muted)">
                    Your browser warns about the certificate because this node signs it with its own
                    authority. Compare the fingerprint with the one on the node console.
                </p>
            </div>

            <p class="su-fp">
                {#each fingerprintLines as line, index (index)}
                    {#if index > 0}<br />{/if}{line}
                {/each}
            </p>

            <a class="cx-btn cx-btn--secondary cx-btn--sm" href={SETUP_CA_URL} download style="justify-self: start">
                <Download class="cx-i" aria-hidden="true" /> Download CA certificate
            </a>

            <fieldset class="cx-field" style="border: 0; margin: 0; padding: 0; min-width: 0">
                <legend class="cx-label" style="padding: 0; margin-bottom: 8px">Setup code from the node console</legend>
                <CodeInput
                    bind:value={setupCode}
                    length={SETUP_CODE_LENGTH}
                    groupSize={4}
                    pattern={REGEXP_ONLY_DIGITS_AND_CHARS}
                    label="Setup code"
                />
            </fieldset>

            <button class="cx-btn cx-btn--primary cx-btn--block" type="submit" disabled={!codeValid}>
                Continue <ChevronRight class="cx-i cx-chev" aria-hidden="true" />
            </button>
            <button class="cx-btn cx-btn--ghost cx-btn--block" type="button" disabled={!codeValid} onclick={() => (step = "join")}>
                Join an existing cluster instead
            </button>
        </form>
    {:else if step === "join"}
        <form
            class="su-pane"
            onsubmit={(event) => {
                event.preventDefault();
                advance();
            }}
        >
            <div style="display: grid; gap: 12px">
                <h1 class="cx-headline display-md">Join a cluster.</h1>
                <p style="margin: 0; font-size: 15px; line-height: 1.6; color: var(--ink-muted)">
                    Paste the join string from Add Node in the web UI of the cluster this node should
                    join. The node reboots after you confirm.
                </p>
            </div>
            <div class="cx-field">
                <label class="cx-label" for="join-string">Join string</label>
                <input
                    class="cx-input"
                    id="join-string"
                    placeholder="csfx-join.host:8000#csfx-bootstrap.&hellip;"
                    autocomplete="off"
                    bind:value={joinString}
                />
            </div>
            <div class="su-actions">
                <button class="cx-btn cx-btn--ghost" type="button" onclick={() => (step = "verify")}>Back</button>
                <button class="cx-btn cx-btn--primary" type="submit" disabled={!joinValid || submitting} aria-busy={submitting}>
                    {#if submitting}<Spinner />{:else}Join cluster <ChevronRight class="cx-i cx-chev" aria-hidden="true" />{/if}
                </button>
            </div>
        </form>
    {:else if step === "joining"}
        <div class="su-pane">
            <h1 class="cx-headline display-md">Joining the cluster.</h1>
            <p class="su-status">
                <Spinner /> This node reboots and continues as a cluster member. Manage it from the
                web UI of the cluster you joined.
            </p>
        </div>
    {:else if step === "account"}
        <form
            class="su-pane"
            onsubmit={(event) => {
                event.preventDefault();
                advance();
            }}
        >
            <h1 class="cx-headline display-md">Create the administrator.</h1>
            <div style="display: grid; gap: 16px">
                <div class="cx-field">
                    <label class="cx-label" for="organization">Organization name</label>
                    <input class="cx-input" id="organization" autocomplete="organization" bind:value={organizationName} />
                </div>
                <div class="cx-field">
                    <label class="cx-label" for="email">Email</label>
                    <input class="cx-input" id="email" type="email" autocomplete="username" placeholder="you@company.de" bind:value={adminEmail} />
                </div>
                <div class="cx-field">
                    <label class="cx-label" for="password">Password</label>
                    <input class="cx-input" id="password" type="password" autocomplete="new-password" placeholder="At least {MIN_PASSWORD_LENGTH} characters" bind:value={password} />
                </div>
                <div class="cx-field">
                    <label class="cx-label" for="password-repeat">Repeat password</label>
                    <input class="cx-input" id="password-repeat" type="password" autocomplete="new-password" aria-invalid={passwordMismatch} bind:value={passwordRepeat} />
                    {#if passwordMismatch}<p class="cx-hint cx-hint--error">Passwords do not match</p>{/if}
                </div>
            </div>
            <div class="su-actions">
                <button class="cx-btn cx-btn--ghost" type="button" onclick={() => (step = "verify")}>Back</button>
                <button class="cx-btn cx-btn--primary" type="submit" disabled={!accountValid}>
                    Continue <ChevronRight class="cx-i cx-chev" aria-hidden="true" />
                </button>
            </div>
        </form>
    {:else if step === "console"}
        <form
            class="su-pane"
            onsubmit={(event) => {
                event.preventDefault();
                advance();
            }}
        >
            <div style="display: grid; gap: 12px">
                <h1 class="cx-headline display-md">Set a console PIN.</h1>
                <p style="margin: 0; font-size: 15px; line-height: 1.6; color: var(--ink-muted)">
                    The PIN protects the recovery menu on the node console. It works without the
                    control plane, so keep it separate from the administrator password.
                </p>
            </div>
            <div style="display: grid; gap: 16px">
                <div class="cx-field">
                    <label class="cx-label" for="pin">PIN</label>
                    <input class="cx-input" id="pin" type="password" autocomplete="off" placeholder="At least {MIN_PIN_LENGTH} characters" bind:value={consolePin} />
                </div>
                <div class="cx-field">
                    <label class="cx-label" for="pin-repeat">Repeat PIN</label>
                    <input class="cx-input" id="pin-repeat" type="password" autocomplete="off" aria-invalid={pinMismatch} bind:value={consolePinRepeat} />
                    {#if pinMismatch}<p class="cx-hint cx-hint--error">PINs do not match</p>{/if}
                </div>
            </div>
            <div class="su-actions">
                <button class="cx-btn cx-btn--ghost" type="button" onclick={() => (step = "account")}>Back</button>
                <button class="cx-btn cx-btn--primary" type="submit" disabled={!pinValid || submitting} aria-busy={submitting}>
                    {#if submitting}<Spinner />{:else}Initialize cluster <ChevronRight class="cx-i cx-chev" aria-hidden="true" />{/if}
                </button>
            </div>
        </form>
    {:else}
        <div class="su-pane">
            <div style="display: grid; gap: 12px">
                <h1 class="cx-headline display-md">Your cluster is ready.</h1>
                <p style="margin: 0; font-size: 15px; line-height: 1.6; color: var(--ink-muted)">
                    Store the console recovery key offline. It replaces the console PIN if you forget
                    it and is shown only once.
                </p>
            </div>
            <p class="su-fp su-fp--key">{recoveryKey}</p>
            <button class="cx-btn cx-btn--secondary cx-btn--block" type="button" onclick={copyRecoveryKey}>
                Copy recovery key
            </button>
            <ul class="su-list">
                <li>Enable two-factor authentication in your account settings.</li>
                <li>SSH access is off. Enable it temporarily under Settings when needed.</li>
                <li>Use the CLI with <span class="cx-mono">csfx login</span>.</li>
            </ul>
            <label class="cx-check">
                <input type="checkbox" bind:checked={keyStored} /> I stored the recovery key
            </label>
            <button class="cx-btn cx-btn--primary cx-btn--block" type="button" disabled={!keyStored} onclick={() => goto("/")}>
                Open dashboard <ChevronRight class="cx-i cx-chev" aria-hidden="true" />
            </button>
        </div>
    {/if}
{/if}
