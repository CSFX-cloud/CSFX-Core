<script lang="ts">
    import { REGEXP_ONLY_DIGITS } from "bits-ui";
    import CodeInput from "$lib/components/auth/code-input.svelte";
    import PasswordInput from "$lib/components/auth/password-input.svelte";
    import SettingsPage from "$lib/components/settings/settings-page.svelte";
    import Switch from "$lib/components/shell/switch.svelte";
    import Spinner from "$lib/components/ui/spinner/spinner.svelte";
    import { auth } from "$lib/auth/store.svelte";
    import {
        changeEmail,
        changeGravatarEmail,
        changePassword,
        gravatarUrl,
        setup2FA,
        enable2FA,
        disable2FA,
    } from "$lib/auth/api";
    import { toast } from "svelte-sonner";

    let email = $state(auth.user?.email ?? "");
    let emailSaving = $state(false);

    let gravatarEmail = $state(auth.user?.gravatar_email ?? "");
    let gravatarSaving = $state(false);
    let avatarPreview = $state<string | null>(null);

    const gravatarChanged = $derived(gravatarEmail.trim() !== (auth.user?.gravatar_email ?? ""));

    $effect(() => {
        const source = auth.user?.gravatar_email;
        if (!source) {
            avatarPreview = null;
            return;
        }
        gravatarUrl(source, 96).then((url) => (avatarPreview = url));
    });

    let oldPassword = $state("");
    let newPassword = $state("");
    let confirmPassword = $state("");
    let passwordSaving = $state(false);

    let twoFactorStep = $state<"idle" | "enrolling" | "disabling">("idle");
    let qrCode = $state("");
    let otpValue = $state("");
    let twoFactorBusy = $state(false);

    const OTP_LENGTH = 6;

    const emailChanged = $derived(email.trim() !== "" && email !== (auth.user?.email ?? ""));
    const passwordValid = $derived(newPassword.length >= 8 && newPassword === confirmPassword);

    async function saveEmail() {
        if (!auth.token || !emailChanged) return;
        emailSaving = true;
        try {
            await changeEmail(auth.token, email.trim());
            auth.setUser({ ...auth.user!, email: email.trim() });
            toast.success("Email updated");
        } catch {
            toast.error("Failed to update email");
        } finally {
            emailSaving = false;
        }
    }

    async function saveGravatarEmail() {
        if (!auth.token || !gravatarChanged) return;
        gravatarSaving = true;
        try {
            const trimmed = gravatarEmail.trim();
            await changeGravatarEmail(auth.token, trimmed === "" ? null : trimmed);
            auth.setUser({ ...auth.user!, gravatar_email: trimmed === "" ? null : trimmed });
            toast.success("Gravatar updated");
        } catch {
            toast.error("Failed to update gravatar");
        } finally {
            gravatarSaving = false;
        }
    }

    async function savePassword() {
        if (!auth.token || !passwordValid) return;
        passwordSaving = true;
        try {
            await changePassword(auth.token, oldPassword, newPassword);
            oldPassword = "";
            newPassword = "";
            confirmPassword = "";
            toast.success("Password updated");
        } catch {
            toast.error("Failed to update password", { description: "Check your current password" });
        } finally {
            passwordSaving = false;
        }
    }

    async function toggleTwoFactor(next: boolean) {
        if (!auth.token) return;
        if (next) {
            twoFactorBusy = true;
            try {
                const response = await setup2FA(auth.token);
                qrCode = response.qr_code;
                twoFactorStep = "enrolling";
            } catch {
                toast.error("Failed to start 2FA setup");
            } finally {
                twoFactorBusy = false;
            }
        } else {
            twoFactorStep = "disabling";
            otpValue = "";
        }
    }

    async function confirmEnable() {
        if (!auth.token || otpValue.length !== OTP_LENGTH) return;
        twoFactorBusy = true;
        try {
            await enable2FA(auth.token, otpValue);
            auth.setUser({ ...auth.user!, two_factor_enabled: true });
            twoFactorStep = "idle";
            otpValue = "";
            toast.success("Two-factor authentication enabled");
        } catch {
            toast.error("Invalid code");
            otpValue = "";
        } finally {
            twoFactorBusy = false;
        }
    }

    async function confirmDisable() {
        if (!auth.token || otpValue.length !== OTP_LENGTH) return;
        twoFactorBusy = true;
        try {
            await disable2FA(auth.token, otpValue);
            auth.setUser({ ...auth.user!, two_factor_enabled: false });
            twoFactorStep = "idle";
            otpValue = "";
            toast.success("Two-factor authentication disabled");
        } catch {
            toast.error("Invalid code");
            otpValue = "";
        } finally {
            twoFactorBusy = false;
        }
    }

    function cancelTwoFactorStep() {
        twoFactorStep = "idle";
        otpValue = "";
        qrCode = "";
    }
</script>

{#snippet otpPanel(note: string, confirmLabel: string, destructive: boolean, onconfirm: () => void)}
    <div style="display: grid; gap: 16px">
        <p class="st-note">{note}</p>
        {#if twoFactorStep === "enrolling" && qrCode}
            <img class="st-qr" src="data:image/png;base64,{qrCode}" alt="2FA QR code" />
        {/if}
        <CodeInput bind:value={otpValue} length={OTP_LENGTH} groupSize={3} pattern={REGEXP_ONLY_DIGITS} label="Authenticator code" />
        <div class="db-actions">
            <button class="cx-btn cx-btn--primary cx-btn--sm {destructive ? 'nd-danger' : ''}" type="button" disabled={otpValue.length !== OTP_LENGTH || twoFactorBusy} aria-busy={twoFactorBusy} onclick={onconfirm}>
                {#if twoFactorBusy}<Spinner />{:else}{confirmLabel}{/if}
            </button>
            <button class="cx-btn cx-btn--ghost cx-btn--sm" type="button" onclick={cancelTwoFactorStep}>Cancel</button>
        </div>
    </div>
{/snippet}

<SettingsPage title="Account" description="Your profile and how you sign in.">
    <section class="cx-panel st-panel" aria-labelledby="st-profile">
        <div><h3 id="st-profile">Profile</h3><p>Shown to other administrators of this cluster.</p></div>
        <div class="st-profile">
            <span class="st-avatar" aria-hidden="true">
                {#if avatarPreview}<img src={avatarPreview} alt="" />{:else}{(auth.user?.username ?? "??").slice(0, 2).toUpperCase()}{/if}
            </span>
            <div style="display: grid; gap: 16px; min-width: 0">
                <div class="cx-field">
                    <label class="cx-label" for="st-grav">Gravatar email</label>
                    <div class="st-row">
                        <input class="cx-input" id="st-grav" type="email" placeholder="you@example.com" aria-describedby="st-grav-hint" bind:value={gravatarEmail} />
                        <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" style="height: 44px" disabled={!gravatarChanged || gravatarSaving} onclick={saveGravatarEmail}>
                            {#if gravatarSaving}<Spinner />{:else}Save{/if}
                        </button>
                    </div>
                    <p class="cx-hint" id="st-grav-hint">
                        The avatar is loaded from <a class="cx-link" href="https://gravatar.com/profile" target="_blank" rel="noreferrer">gravatar.com</a>. Leave empty to keep initials.
                    </p>
                </div>
                <div class="cx-field">
                    <label class="cx-label" for="st-user">Username</label>
                    <input class="cx-input st-ro" id="st-user" type="text" value={auth.user?.username ?? ""} readonly aria-describedby="st-user-hint" />
                    <p class="cx-hint" id="st-user-hint">Set during setup. It can't be changed.</p>
                </div>
                <div class="cx-field">
                    <label class="cx-label" for="st-mail">Email</label>
                    <div class="st-row">
                        <input class="cx-input" id="st-mail" type="email" autocomplete="email" placeholder="you@example.com" bind:value={email} />
                        <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" style="height: 44px" disabled={!emailChanged || emailSaving} onclick={saveEmail}>
                            {#if emailSaving}<Spinner />{:else}Save{/if}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    </section>

    <section class="cx-panel st-panel" aria-labelledby="st-pw">
        <div><h3 id="st-pw">Password</h3><p>Use one you don't use anywhere else.</p></div>
        <form
            style="display: grid; gap: 16px"
            onsubmit={(event) => {
                event.preventDefault();
                savePassword();
            }}
        >
            <div class="cx-field">
                <label class="cx-label" for="st-cur">Current password</label>
                <PasswordInput id="st-cur" autocomplete="current-password" bind:value={oldPassword} />
            </div>
            <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 16px">
                <div class="cx-field">
                    <label class="cx-label" for="st-new">New password</label>
                    <PasswordInput id="st-new" autocomplete="new-password" bind:value={newPassword} />
                </div>
                <div class="cx-field">
                    <label class="cx-label" for="st-conf">Confirm new password</label>
                    <PasswordInput id="st-conf" autocomplete="new-password" bind:value={confirmPassword} />
                </div>
            </div>
            <div>
                <button class="cx-btn cx-btn--primary cx-btn--sm" type="submit" disabled={!passwordValid || passwordSaving} aria-busy={passwordSaving}>
                    {#if passwordSaving}<Spinner />{:else}Update password{/if}
                </button>
            </div>
        </form>
    </section>

    <section class="cx-panel st-panel" aria-labelledby="st-2fa">
        <div class="st-toggle">
            <div>
                <h3 id="st-2fa">Two-factor authentication</h3>
                <p>Ask for a code from an authenticator app at every sign-in.</p>
            </div>
            <Switch
                checked={auth.user?.two_factor_enabled ?? false}
                label="Two-factor authentication"
                disabled={twoFactorBusy || twoFactorStep !== "idle"}
                onchange={toggleTwoFactor}
            />
        </div>
        {#if twoFactorStep === "enrolling"}
            {@render otpPanel("Scan the QR code with your authenticator app, then enter the code.", "Confirm", false, confirmEnable)}
        {:else if twoFactorStep === "disabling"}
            {@render otpPanel("Enter your current code to disable two-factor authentication.", "Disable", true, confirmDisable)}
        {/if}
    </section>
</SettingsPage>
