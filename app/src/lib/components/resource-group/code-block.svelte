<script lang="ts">
    import { createClipboard } from "$lib/utils/clipboard.svelte";
    import { highlightCode, type CodeLanguage } from "$lib/utils/code-highlight";

    let { filename, tag, language, code }: { filename: string; tag: string; language: CodeLanguage; code: string } = $props();

    const clipboard = createClipboard();
    const highlighted = $derived(highlightCode(code, language));
    const lineNumbers = $derived(Array.from({ length: code.split("\n").length }, (_, index) => index + 1).join("\n"));
</script>

<section class="cx-panel cb" aria-label={filename}>
    <div class="cb__head">
        <div class="cb__title">
            <span class="cb__name">{filename}</span>
            <span class="cb__tag">{tag}</span>
        </div>
        <button class="cx-btn cx-btn--secondary cx-btn--sm" type="button" onclick={() => clipboard.copy(code, filename)}>
            {clipboard.copiedKey === filename ? "Copied" : "Copy"}
        </button>
    </div>
    <div class="cb__body">
        <pre class="cb__nums" aria-hidden="true">{lineNumbers}</pre>
        <pre class="cb__code"><code>{@html highlighted}</code></pre>
    </div>
</section>
