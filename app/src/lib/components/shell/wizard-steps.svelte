<script lang="ts">
    type Props = { steps: string[]; current: number; label: string };

    let { steps, current, label }: Props = $props();
</script>

<ol class="wz-steps" aria-label={label}>
    {#each steps as step, index (step)}
        <li
            class="wz-step"
            class:wz-step--done={index < current}
            class:wz-step--active={index === current}
            aria-current={index === current ? "step" : undefined}
        >
            <span class="wz-step__bar"></span>
            <span class="wz-step__label"><span class="wz-step__index">{index + 1}</span>{step}</span>
        </li>
    {/each}
</ol>
