<script lang="ts">
    import { Button } from "$lib/components/ui/button/index.js";
    import { Input } from "$lib/components/ui/input/index.js";
    import { Label } from "$lib/components/ui/label/index.js";
    import { Lock, User, CircleCheck } from "@lucide/svelte";
    import { onMount } from "svelte";
    import { goto } from "$app/navigation";
    import { page } from "$app/state";
    import {
        listSsoProviders,
        login,
        ssoStartUrl,
        TwoFactorRequiredError,
        type SsoProvider,
    } from "$lib/auth/api";
    import { pendingLogin } from "$lib/auth/pending-login";
    import { auth } from "$lib/auth/store.svelte";
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

    onMount(async () => {
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

<img
    src="/logo/logo-csfx.svg"
    alt="CSFX Logo"
    class="size-20 mb-6 rounded-md p-2 invert dark:invert-0"
/>
<h1 class="text-2xl font-light mb-10">Sign in</h1>

<form
    onsubmit={(e) => {
        e.preventDefault();
        handleSubmit();
    }}
    class="flex flex-col"
>
    <div class="flex flex-col gap-1 mb-4">
        <Label for="username" class="text-xs font-bold mb-1">Username</Label>
        <div class="relative">
            <User
                class="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground size-4"
            />
            <Input
                id="username"
                type="text"
                placeholder="admin"
                class="pl-9"
                bind:value={username}
                autocomplete="username"
            />
        </div>
    </div>

    <div class="flex flex-col gap-1 mb-6">
        <Label for="password" class="text-xs font-bold mb-1">Password</Label>
        <div class="relative">
            <Lock
                class="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground"
            />
            <Input
                id="password"
                type="password"
                placeholder="••••••••"
                class="pl-9"
                bind:value={password}
                autocomplete="current-password"
            />
        </div>
    </div>

    <Button
        class="w-full"
        type="submit"
        disabled={status !== "idle" || !username || !password}
    >
        {#if status === "loading"}
            <Spinner />
        {:else if status === "success"}
            <CircleCheck class="size-4 animate-in zoom-in-50 duration-300" />
        {:else}
            Sign in
        {/if}
    </Button>
</form>

{#if ssoProviders.length > 0}
    <div class="flex items-center gap-3 my-6 text-xs text-muted-foreground">
        <div class="h-px flex-1 bg-border"></div>
        or
        <div class="h-px flex-1 bg-border"></div>
    </div>
    <div class="flex flex-col gap-2">
        {#each ssoProviders as provider (provider.slug)}
            <Button variant="outline" class="w-full" href={ssoStartUrl(provider.slug)}>
                Continue with {provider.display_name}
            </Button>
        {/each}
    </div>
{/if}
