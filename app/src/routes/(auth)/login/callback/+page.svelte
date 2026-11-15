<script lang="ts">
    import { onMount } from "svelte";
    import { goto } from "$app/navigation";
    import { page } from "$app/state";
    import { exchangeSsoCode } from "$lib/auth/api";
    import Spinner from "$lib/components/ui/spinner/spinner.svelte";

    onMount(async () => {
        const code = page.url.searchParams.get("code");
        if (!code) {
            goto("/login?sso_error=invalid_state");
            return;
        }
        try {
            await exchangeSsoCode(code);
            goto("/", { invalidateAll: true });
        } catch {
            goto("/login?sso_error=invalid_state");
        }
    });
</script>

<div class="flex flex-col items-center gap-4 py-10">
    <Spinner />
    <p class="text-sm text-muted-foreground">Completing sign-in</p>
</div>
