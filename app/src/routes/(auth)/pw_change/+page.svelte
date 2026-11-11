<script lang="ts">
    import { Button } from "$lib/components/ui/button/index.js";
    import { Input } from "$lib/components/ui/input/index.js";
    import { Label } from "$lib/components/ui/label/index.js";
    import { Lock, CircleCheck } from "@lucide/svelte";
    import { goto } from "$app/navigation";
    import { changePassword } from "$lib/auth/api";
    import { auth } from "$lib/auth/store.svelte";
    import Spinner from "$lib/components/ui/spinner/spinner.svelte";
    import { toast } from "svelte-sonner";

    type Status = "idle" | "loading" | "success";

    let newPassword = $state("");
    let confirmPassword = $state("");
    let status = $state<Status>("idle");

    const mismatch = $derived(confirmPassword.length > 0 && newPassword !== confirmPassword);
    const canSubmit = $derived(newPassword.length >= 8 && newPassword === confirmPassword && status === "idle");

    async function handleSubmit() {
        if (!canSubmit || !auth.token) return;
        status = "loading";
        try {
            auth.setToken(await changePassword(auth.token, "", newPassword));
            status = "success";
            setTimeout(() => goto("/"), 600);
        } catch {
            status = "idle";
            toast.error("Failed to change password", {
                description: "Please try again",
            });
        }
    }
</script>

<img
    src="/logo/logo-csfx.svg"
    alt="CSFX Logo"
    class="size-20 mb-6 rounded-md p-2 invert dark:invert-0"
/>
<h1 class="text-2xl font-light mb-2">Change Password</h1>
<p class="text-sm text-muted-foreground mb-10">A new password is required before continuing</p>

<form onsubmit={(e) => { e.preventDefault(); handleSubmit(); }} class="flex flex-col">
    <div class="flex flex-col gap-1 mb-4">
        <Label for="new-password" class="text-xs font-bold mb-1">New Password</Label>
        <div class="relative">
            <Lock class="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
            <Input
                id="new-password"
                type="password"
                placeholder="••••••••"
                class="pl-9"
                bind:value={newPassword}
                autocomplete="new-password"
            />
        </div>
    </div>

    <div class="flex flex-col gap-1 mb-6">
        <Label for="confirm-password" class="text-xs font-bold mb-1">Confirm Password</Label>
        <div class="relative">
            <Lock class="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
            <Input
                id="confirm-password"
                type="password"
                placeholder="••••••••"
                class="pl-9 {mismatch ? 'border-destructive' : ''}"
                bind:value={confirmPassword}
                autocomplete="new-password"
            />
        </div>
        {#if mismatch}
            <p class="text-xs text-destructive mt-1">Passwords do not match</p>
        {/if}
    </div>

    <Button class="w-full" type="submit" disabled={!canSubmit}>
        {#if status === "loading"}
            <Spinner />
        {:else if status === "success"}
            <CircleCheck class="size-4 animate-in zoom-in-50 duration-300" />
        {:else}
            Change Password
        {/if}
    </Button>
</form>
