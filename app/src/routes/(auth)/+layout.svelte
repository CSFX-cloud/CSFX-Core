<script lang="ts">
    import "@fontsource-variable/inter/index.css";
    import "$lib/styles/csfx-design-system.css";
    import "$lib/styles/csfx-setup.css";
    import type { Snippet } from "svelte";
    import AuthStage from "$lib/components/auth/auth-stage.svelte";

    let { children }: { children: Snippet } = $props();
</script>

<div class="cx-root" style="min-height: 100vh">
    <main class="cx-auth">
        <section class="cx-card cx-auth__panel" aria-label="Account access">
            <div class="cx-auth__top">
                <span class="cx-wordmark">
                    <img class="cx-wordmark__logo" src="/logo/logo-csfx.svg" alt="" />
                    csfx.cloud
                </span>
            </div>
            <div class="cx-auth__body">
                <div class="cx-auth__form" style="gap: 32px">
                    {@render children()}
                </div>
            </div>
        </section>
        <AuthStage />
    </main>
</div>
