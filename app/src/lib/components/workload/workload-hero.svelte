<script lang="ts">
    import type { Component, Snippet } from "svelte";
    import type { Server } from "@lucide/svelte";
    import { badgeClass, statusLabel } from "$lib/utils/tone";

    export interface MetaItem {
        icon: typeof Server | Component;
        text: string;
        href?: string;
    }

    type Props = {
        title: string;
        status: string;
        statusText?: string;
        meta: MetaItem[];
        label: string;
        lead: Snippet;
        actions: Snippet;
    };

    let { title, status, statusText, meta, label, lead, actions }: Props = $props();
</script>

<section class="cx-stage" aria-label={label}>
    <div class="cx-stage__canvas"></div>
    <div class="rd-head">
        <div class="rd-id">
            {@render lead()}
            <div style="min-width: 0">
                <div class="rd-title">
                    <h1>{title}</h1>
                    <span class={badgeClass(status)}>{statusText ?? statusLabel(status)}</span>
                </div>
                <div class="rd-meta">
                    {#each meta as item (item.text)}
                        <span>
                            <item.icon class="cx-i" aria-hidden="true" />
                            {#if item.href}<a href={item.href}>{item.text}</a>{:else}{item.text}{/if}
                        </span>
                    {/each}
                </div>
            </div>
        </div>
        <div class="db-actions">{@render actions()}</div>
    </div>
</section>
