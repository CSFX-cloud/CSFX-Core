<script lang="ts">
    import { ChevronRight, CircleCheck } from "@lucide/svelte";
    import { onMount } from "svelte";
    import Icon from "@iconify/svelte";
    import { presetFor } from "$lib/components/settings/identity-provider-presets";
    import { goto } from "$app/navigation";
    import { page } from "$app/state";
    import {
        getSetupStatus,
        listSsoProviders,
        login,
        ssoStartUrl,
        TwoFactorRequiredError,
        type SsoProvider,
    } from "$lib/auth/api";
    import { pendingLogin } from "$lib/auth/pending-login";
    import { auth } from "$lib/auth/store.svelte";
    import PasswordInput from "$lib/components/auth/password-input.svelte";
    import Spinner from "$lib/components/ui/spinner/spinner.svelte";
    import { toast } from "svelte-sonner";

    type Status = "idle" | "loading" | "success";

    let username = $state("");
    let password = $state("");
    let status = $state<Status>("idle");
    let ssoProviders = $state<SsoProvider[]>([]);

    const SSO_ERROR_MESSAGES: Record<string, string> = {
        no_role: "Your account has no role assigned for this application",
        not_provisioned: "Your account is not allowed to sign in here",
        username_conflict: "A user with this name already exists",
        provider_denied: "The identity provider denied the sign-in",
        provider_unavailable: "The identity provider is unavailable",
        invalid_state: "The sign-in session expired, try again",
        unknown_provider: "This identity provider is not available",
    };

    let starting = $state(false);

    async function redirectToSetupWhenRequired() {
        try {
            const setup = await getSetupStatus();
            starting = false;
            if (setup.state === "setup_required") goto("/setup");
        } catch {
            starting = true;
            setTimeout(redirectToSetupWhenRequired, 5000);
        }
    }

    onMount(async () => {
        redirectToSetupWhenRequired();
        const ssoError = page.url.searchParams.get("sso_error");
        if (ssoError) {
            toast.error("Single sign-on failed", {
                description: SSO_ERROR_MESSAGES[ssoError] ?? "Sign-in could not be completed",
            });
        }
        try {
            ssoProviders = await listSsoProviders();
        } catch {
            ssoProviders = [];
        }
    });

    async function handleSubmit() {
        status = "loading";
        try {
            const response = await login(username, password);
            auth.setSession(response);
            status = "success";
            const target = response.force_password_change ? "/pw_change" : "/";
            setTimeout(() => goto(target), 600);
        } catch (err) {
            status = "idle";
            if (err instanceof TwoFactorRequiredError) {
                pendingLogin.set(err.username, err.password);
                goto("/otp");
            } else {
                toast.error("Invalid credentials", {
                    description:
                        "Check your username and password and try again",
                });
            }
        }
    }
</script>

<form
    class="su-pane"
    onsubmit={(event) => {
        event.preventDefault();
        handleSubmit();
    }}
>
    <h1 class="cx-headline display-md">Sign in.</h1>

    {#if starting}
        <p class="su-status">
            <Spinner /> Control plane is starting, this can take a few minutes.
        </p>
    {/if}

    <div style="display: grid; gap: 16px">
        <div class="cx-field">
            <label class="cx-label" for="username">Username</label>
            <input
                class="cx-input"
                id="username"
                type="text"
                placeholder="admin"
                autocomplete="username"
                bind:value={username}
            />
        </div>
        <div class="cx-field">
            <label class="cx-label" for="password">Password</label>
            <PasswordInput
                id="password"
                autocomplete="current-password"
                placeholder="&bull;&bull;&bull;&bull;&bull;&bull;&bull;&bull;"
                bind:value={password}
            />
        </div>
    </div>

    <button
        class="cx-btn cx-btn--primary cx-btn--block"
        type="submit"
        disabled={status !== "idle" || !username || !password}
        aria-busy={status === "loading"}
    >
        {#if status === "loading"}
            <Spinner />
        {:else if status === "success"}
            <CircleCheck class="cx-i" aria-hidden="true" />
        {:else}
            Sign in <ChevronRight class="cx-i cx-chev" aria-hidden="true" />
        {/if}
    </button>

    {#if ssoProviders.length > 0}
        <div class="cx-divider">or</div>
        <div style="display: grid; gap: 8px">
            {#each ssoProviders as provider (provider.slug)}
                {@const preset = presetFor(provider.slug, provider.display_name)}
                <a class="cx-btn cx-btn--secondary cx-btn--block" href={ssoStartUrl(provider.slug)}>
                    <Icon icon={preset.icon} width={16} height={16} color={preset.iconColor} />
                    Continue with {provider.display_name}
                </a>
            {/each}
        </div>
    {/if}
</form>
