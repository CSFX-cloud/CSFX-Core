<script lang="ts">
    import type { Snippet } from "svelte";

    let { children }: { children: Snippet } = $props();
</script>

<div class="relative flex h-screen w-full items-center justify-start overflow-hidden">
    <img
        src="/login/login-5120.avif"
        alt=""
        class="absolute inset-0 h-full w-full object-cover"
        aria-hidden="true"
    />

    <div
        class="relative z-10 ml-16 flex h-[92%] w-full max-w-2xl flex-col items-center justify-center rounded-2xl bg-card p-8 shadow-2xl"
    >
        <div class="w-full max-w-sm">
            {@render children()}
        </div>
    </div>
</div>
