<script lang="ts">
    import { Check, Copy } from "@lucide/svelte";
    import { page } from "$app/stores";
    import type { SiteDetail } from "$lib/api/sites";
    import { createClipboard } from "$lib/utils/clipboard.svelte";

    let { site }: { site: SiteDetail } = $props();

    const clipboard = createClipboard();
    const url = $derived(`${$page.url.origin}/api${site.webhook_path}`);
</script>

<section class="cx-panel" aria-labelledby="wh-title">
    <div class="cx-panel__head">
        <div>
            <h2 class="cx-panel__title" id="wh-title">Deploy hook</h2>
            <p class="cx-panel__sub">Add this URL as a webhook for push and pull request events in GitHub, Forgejo or Gitea. Content type: JSON.</p>
        </div>
    </div>
    <div style="display: flex; gap: 8px; align-items: center; padding: 0 24px 24px">
        <code class="kv-name" style="overflow-wrap: anywhere">{url}</code>
        <button class="cx-icon-btn cx-icon-btn--sm" type="button" aria-label="Copy deploy hook" title="Copy" onclick={() => clipboard.copy(url, "hook")}>
            {#if clipboard.copiedKey === "hook"}<Check class="cx-i" aria-hidden="true" />{:else}<Copy class="cx-i" aria-hidden="true" />{/if}
        </button>
    </div>
</section>
