<script lang="ts">
    import { Button } from "$lib/components/ui/button/index.js";
    import { Input } from "$lib/components/ui/input/index.js";
    import { Label } from "$lib/components/ui/label/index.js";
    import { Lock, User, CircleCheck } from "@lucide/svelte";
    import { goto } from "$app/navigation";
    import { login, TwoFactorRequiredError } from "$lib/auth/api";
    import { pendingLogin } from "$lib/auth/pending-login";
    import { auth } from "$lib/auth/store.svelte";
    import Spinner from "$lib/components/ui/spinner/spinner.svelte";
    import { toast } from "svelte-sonner";

    type Status = "idle" | "loading" | "success";

    let username = $state("");
    let password = $state("");
    let status = $state<Status>("idle");

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
