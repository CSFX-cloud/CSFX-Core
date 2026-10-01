<script lang="ts">
    import { Button } from "$lib/components/ui/button/index.js";
    import { Label } from "$lib/components/ui/label/index.js";
    import { CircleCheck } from "@lucide/svelte";
    import { goto } from "$app/navigation";
    import { onDestroy, onMount } from "svelte";
    import * as InputOTP from "$lib/components/ui/input-otp/index.js";
    import { login } from "$lib/auth/api";
    import { pendingLogin } from "$lib/auth/pending-login";
    import { auth } from "$lib/auth/store.svelte";
    import Spinner from "$lib/components/ui/spinner/spinner.svelte";
    import { toast } from "svelte-sonner";

    type Status = "idle" | "loading" | "success";

    let otpValue = $state("");
    let status = $state<Status>("idle");

    onMount(() => {
        if (!pendingLogin.get()) goto("/login");
    });

    onDestroy(() => pendingLogin.clear());

    async function handleVerify() {
        const credentials = pendingLogin.get();
        if (otpValue.length !== 6 || !credentials) return;
        status = "loading";
        try {
            const response = await login(credentials.username, credentials.password, otpValue);
            pendingLogin.clear();
            auth.setSession(response);
            status = "success";
            const target = response.force_password_change ? "/pw_change" : "/";
            setTimeout(() => goto(target), 600);
        } catch {
            status = "idle";
            otpValue = "";
            toast.error("Invalid code", {
                description: "Check the code and try again",
            });
        }
    }

    $effect(() => {
        if (otpValue.length === 6) handleVerify();
    });
</script>

<img
    src="/logo/logo-csfx.svg"
    alt="CSFX Logo"
    class="size-20 mb-6 rounded-md p-2 invert dark:invert-0"
/>
<h1 class="text-2xl font-light mb-2">2FA Verification</h1>
<p class="text-sm text-muted-foreground mb-10">Enter the 6-digit code from your authenticator app</p>

<form onsubmit={(e) => { e.preventDefault(); handleVerify(); }} class="flex flex-col">
    <div class="flex flex-col gap-1 mb-20">
        <Label class="text-xs font-bold mb-1">TOTP</Label>
        <InputOTP.Root maxlength={6} class="w-full" bind:value={otpValue}>
            {#snippet children({ cells })}
                <InputOTP.Group class="flex-1">
                    {#each cells.slice(0, 3) as cell (cell)}
                        <InputOTP.Slot {cell} class="flex-1 w-full" />
                    {/each}
                </InputOTP.Group>
                <InputOTP.Separator />
                <InputOTP.Group class="flex-1">
                    {#each cells.slice(3, 6) as cell (cell)}
                        <InputOTP.Slot {cell} class="flex-1 w-full" />
                    {/each}
                </InputOTP.Group>
            {/snippet}
        </InputOTP.Root>
    </div>

    <Button class="w-full" type="submit" disabled={status !== "idle" || otpValue.length !== 6}>
        {#if status === "loading"}
            <Spinner />
        {:else if status === "success"}
            <CircleCheck class="size-4 animate-in zoom-in-50 duration-300" />
        {:else}
            Verify
        {/if}
    </Button>
</form>
