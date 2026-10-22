<script lang="ts">
    import { PinInput } from "bits-ui";

    type Props = {
        value?: string;
        length: number;
        groupSize: number;
        pattern?: string;
        label: string;
    };

    let { value = $bindable(""), length, groupSize, pattern, label }: Props = $props();

    function sanitize(text: string) {
        return text.replace(/[^a-z0-9]/gi, "");
    }
</script>

<PinInput.Root
    bind:value
    maxlength={length}
    {pattern}
    class="su-code"
    aria-label={label}
    pasteTransformer={sanitize}
>
    {#snippet children({ cells })}
        {#each { length: Math.ceil(cells.length / groupSize) } as _, group (group)}
            {#if group > 0}<span class="su-code__dash" aria-hidden="true"></span>{/if}
            <div class="su-code__group">
                {#each cells.slice(group * groupSize, (group + 1) * groupSize) as cell (cell)}
                    <PinInput.Cell {cell} class="cx-input su-cell">
                        {cell.char}
                        {#if cell.hasFakeCaret}<span class="su-caret" aria-hidden="true"></span>{/if}
                    </PinInput.Cell>
                {/each}
            </div>
        {/each}
    {/snippet}
</PinInput.Root>
