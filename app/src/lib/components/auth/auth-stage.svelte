<script lang="ts">
    import { onMount } from "svelte";
    import { ChevronLeft, ChevronRight, RefreshCw } from "@lucide/svelte";

    type Tone = "success" | "warning" | "danger";

    const SLIDE_COUNT = 4;
    const SLIDE_INTERVAL_MS = 6500;
    const TICK_MS = 250;
    const POINT_COUNT = 56;
    const CHART_WIDTH = 600;
    const CHART_BASELINE = 238;
    const CHART_HEIGHT = 236;

    const NODES: { name: string; tone: Tone; label: string; region: string; cpu: number }[] = [
        { name: "fra-node-01", tone: "success", label: "Healthy", region: "Frankfurt", cpu: 34 },
        { name: "ber-node-02", tone: "warning", label: "Warning", region: "Berlin", cpu: 71 },
        { name: "nue-node-03", tone: "success", label: "Healthy", region: "Nuremberg", cpu: 48 },
        { name: "muc-node-04", tone: "success", label: "Healthy", region: "Munich", cpu: 22 },
        { name: "fra-node-07", tone: "danger", label: "Critical", region: "Frankfurt", cpu: 96 },
    ];

    const TASKS: { label: string; time: string; state: "done" | "running" | "queued" }[] = [
        { label: "Node fra-node-08 provisioned", time: "4s", state: "done" },
        { label: "Tenant keys rotated in HSM", time: "3s", state: "done" },
        { label: "GDPR audit log sealed", time: "10s", state: "done" },
        { label: "Snapshot policy pinned to eu-central", time: "2s", state: "done" },
        { label: "Migrating vm-204 to ber-node-02…", time: "2s", state: "running" },
        { label: "Backup target verified in Nuremberg", time: "", state: "queued" },
    ];

    const ALERTS: { tone: Tone; title: string; body: string; time: string; fresh: boolean }[] = [
        { tone: "danger", title: "High CPU usage", body: "fra-node-07 CPU usage exceeded 95%", time: "1 min ago", fresh: true },
        { tone: "warning", title: "Data residency check", body: "vm-218 snapshot target outside EU zone blocked", time: "3 min ago", fresh: false },
        { tone: "success", title: "GDPR audit log sealed", body: "Daily audit chain signed and anchored", time: "12 min ago", fresh: false },
    ];

    const CAPTIONS = [
        { title: "See every node at a glance", text: "from healthy to critical, every status is visible, traceable and actionable, without telemetry ever leaving your cluster." },
        { title: "Real-time resource monitoring", text: "CPU, memory, disk and network across every node and VM, streamed live from your own hardware." },
        { title: "Sovereign orchestration", text: "every node, VM and volume is pinned to the region you choose. Policies are enforced by the orchestrator, not by a checkbox." },
        { title: "Sovereignty alerts", text: "get notified the moment a workload, snapshot or backup would leave its designated region." },
    ];

    function buildSeries() {
        let seed = 7;
        const random = () => {
            seed = (seed * 9301 + 49297) % 233280;
            return seed / 233280;
        };
        const cpu: number[] = [];
        const memory: number[] = [];
        for (let index = 0; index < POINT_COUNT; index++) {
            const progress = index / (POINT_COUNT - 1);
            cpu.push(44 + progress * 36 + (random() - 0.5) * 12 + Math.sin(index * 1.3) * 3);
            memory.push(42 + progress * 8 + (random() - 0.5) * 10 + Math.sin(index * 1.7) * 2);
        }
        return { cpu, memory };
    }

    function linePath(series: number[]) {
        return series
            .map((value, index) => {
                const x = (CHART_WIDTH * index) / (POINT_COUNT - 1);
                const y = CHART_BASELINE - (CHART_HEIGHT * value) / 100;
                return `${index ? "L" : "M"}${x.toFixed(1)} ${y.toFixed(1)}`;
            })
            .join("");
    }

    function areaPath(series: number[]) {
        return `${linePath(series)}L${CHART_WIDTH} ${CHART_BASELINE}L0 ${CHART_BASELINE}Z`;
    }

    const series = buildSeries();
    const cpuLine = linePath(series.cpu);
    const memoryLine = linePath(series.memory);
    const cpuArea = areaPath(series.cpu);
    const memoryArea = areaPath(series.memory);

    let slide = $state(0);
    let paused = $state(false);
    let reducedMotion = $state(false);
    let lastTick = Date.now();

    function goTo(index: number) {
        lastTick = Date.now();
        slide = (index + SLIDE_COUNT) % SLIDE_COUNT;
    }

    function pause() {
        paused = true;
    }

    function resume() {
        lastTick = Date.now();
        paused = false;
    }

    onMount(() => {
        reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
        if (reducedMotion) return;
        const timer = setInterval(() => {
            if (paused) {
                lastTick = Date.now();
                return;
            }
            if (Date.now() - lastTick >= SLIDE_INTERVAL_MS) goTo(slide + 1);
        }, TICK_MS);
        return () => clearInterval(timer);
    });
</script>

<section
    class="cx-stage cx-auth__stage {paused ? 'is-paused' : ''}"
    aria-roledescription="carousel"
    aria-label="What csfx.cloud does"
    onmouseenter={pause}
    onmouseleave={resume}
    onfocusin={pause}
    onfocusout={resume}
>
    <div class="cx-stage__canvas"></div>

    <div class="cx-window cx-window--bleed-right cx-float su-window">
        {#key slide}
            <div class="su-slide" role="group" aria-roledescription="slide" aria-label="{slide + 1} of {SLIDE_COUNT}">
                {#if slide === 0}
                    <div class="cx-panel__head">
                        <div>
                            <h2 class="cx-panel__title">Node status</h2>
                            <p class="cx-panel__sub">Hypervisor nodes across German regions</p>
                        </div>
                        <span class="cx-panel__meta"><RefreshCw class="cx-i" aria-hidden="true" />Refreshed 5 min ago</span>
                    </div>
                    <table class="cx-table">
                        <thead><tr><th>Node</th><th>Status</th><th>Region</th><th>CPU</th></tr></thead>
                        <tbody>
                            {#each NODES as node (node.name)}
                                <tr>
                                    <td>{node.name}</td>
                                    <td><span class="cx-badge cx-badge--{node.tone}">{node.label}</span></td>
                                    <td class="cx-soft">{node.region}</td>
                                    <td>
                                        <div class="cx-usage {node.tone === 'success' ? '' : `cx-usage--${node.tone}`}">
                                            <div class="cx-usage__track"><div class="cx-usage__fill" style="width: {node.cpu}%"></div></div>
                                            <span class="cx-usage__value">{node.cpu}%</span>
                                        </div>
                                    </td>
                                </tr>
                            {/each}
                        </tbody>
                    </table>
                {:else if slide === 1}
                    <div class="cx-panel__head">
                        <div>
                            <h2 class="cx-panel__title">CPU &amp; memory usage</h2>
                            <p class="cx-panel__sub">Cluster-wide utilisation across all nodes</p>
                        </div>
                        <div class="su-range"><span>1H</span><span class="on">24H</span><span>7D</span></div>
                    </div>
                    <div class="su-legend">
                        <span><i style="background: #2dd4bf"></i>CPU usage</span>
                        <span><i style="background: #3b82f6"></i>Memory usage</span>
                    </div>
                    <div class="su-chart">
                        <div class="su-chart__y" aria-hidden="true">
                            <span>100%</span><span>75%</span><span>50%</span><span>25%</span><span>0</span>
                        </div>
                        <svg viewBox="0 0 600 240" preserveAspectRatio="none" role="img" aria-label="CPU rising from 45 to 82 percent, memory steady around 50 percent">
                            <defs>
                                <linearGradient id="su-gradient-cpu" x1="0" y1="0" x2="0" y2="1">
                                    <stop offset="0" style="stop-color: #2dd4bf; stop-opacity: 0.3"></stop>
                                    <stop offset="1" style="stop-color: #2dd4bf; stop-opacity: 0"></stop>
                                </linearGradient>
                                <linearGradient id="su-gradient-memory" x1="0" y1="0" x2="0" y2="1">
                                    <stop offset="0" style="stop-color: #3b82f6; stop-opacity: 0.4"></stop>
                                    <stop offset="1" style="stop-color: #3b82f6; stop-opacity: 0"></stop>
                                </linearGradient>
                            </defs>
                            <path class="su-grid" d="M0 2H600M0 61H600M0 120H600M0 179H600M0 238H600"></path>
                            <path d={cpuArea} fill="url(#su-gradient-cpu)"></path>
                            <path d={memoryArea} fill="url(#su-gradient-memory)"></path>
                            <path class="su-line" d={cpuLine} style="stroke: #2dd4bf"></path>
                            <path class="su-line" d={memoryLine} style="stroke: #3b82f6"></path>
                        </svg>
                    </div>
                {:else if slide === 2}
                    <div class="cx-panel__head">
                        <div>
                            <h2 class="cx-panel__title">Orchestration</h2>
                            <p class="cx-panel__sub">Policy run on eu-central-sovereign</p>
                        </div>
                        <div class="cx-tasks__head" style="margin: 0">
                            <div class="cx-tasks__bar"><span style="width: 40%"></span></div>
                            <span class="cx-tasks__count"><b>4</b> / 10 Completed</span>
                        </div>
                    </div>
                    <div style="padding-top: 8px">
                        {#each TASKS as task (task.label)}
                            <div class="cx-task {task.state === 'done' ? '' : `cx-task--${task.state}`}">
                                <span class="cx-task__icon">
                                    <svg viewBox="0 0 24 24" aria-hidden="true">
                                        {#if task.state === "done"}
                                            <circle cx="12" cy="12" r="10" style="fill: #4ade80"></circle>
                                            <path d="m8 12.5 2.6 2.5L16 9.5" style="fill: none; stroke: #0a0a0a; stroke-width: 2.2; stroke-linecap: round; stroke-linejoin: round"></path>
                                        {:else if task.state === "running"}
                                            <path d="M21 12a9 9 0 1 1-6.22-8.56" style="fill: none; stroke: #8a8a8a; stroke-width: 2; stroke-linecap: round"></path>
                                        {:else}
                                            <circle cx="12" cy="12" r="9" style="fill: none; stroke: #3a3a3a; stroke-width: 2"></circle>
                                        {/if}
                                    </svg>
                                </span>
                                <span class="cx-task__label">{task.label}</span>
                                <span class="cx-task__time">{task.time}</span>
                            </div>
                        {/each}
                    </div>
                {:else}
                    <div class="cx-panel__head">
                        <div>
                            <h2 class="cx-panel__title">Recent alerts</h2>
                            <p class="cx-panel__sub">Sovereignty, compliance and capacity events</p>
                        </div>
                    </div>
                    <div style="display: grid; gap: 10px">
                        {#each ALERTS as alert (alert.title)}
                            <div class="cx-alert cx-alert--{alert.tone}">
                                <span class="cx-alert__dot"></span>
                                <span class="cx-alert__title">{alert.title}</span>
                                {#if alert.fresh}<span class="cx-alert__new">NEW</span>{:else}<span></span>{/if}
                                <span class="cx-alert__body">{alert.body}</span>
                                <span class="cx-alert__time">{alert.time}</span>
                            </div>
                        {/each}
                    </div>
                {/if}
            </div>
        {/key}
    </div>

    <div class="su-cap">
        {#key slide}
            <p class="cx-lead cx-muted"><strong>{CAPTIONS[slide].title}</strong> &mdash; {CAPTIONS[slide].text}</p>
        {/key}

        <div class="su-controls">
            <div class="su-controls__left">
                <div class="cx-dots {reducedMotion ? 'cx-dots--static' : ''}" style="--cx-dur: {SLIDE_INTERVAL_MS}ms">
                    {#each CAPTIONS as _, index (index)}
                        <button
                            class="cx-dot"
                            type="button"
                            aria-label="Show feature {index + 1}"
                            aria-current={index === slide}
                            onclick={() => goTo(index)}
                        ></button>
                    {/each}
                </div>
                <span class="cx-counter"><b>0{slide + 1}</b> / 0{SLIDE_COUNT}</span>
            </div>
            <div class="cx-carousel-nav">
                <button class="cx-icon-btn" type="button" aria-label="Previous feature" onclick={() => goTo(slide - 1)}>
                    <ChevronLeft class="cx-i" aria-hidden="true" />
                </button>
                <button class="cx-icon-btn" type="button" aria-label="Next feature" onclick={() => goTo(slide + 1)}>
                    <ChevronRight class="cx-i" aria-hidden="true" />
                </button>
            </div>
        </div>
    </div>
</section>
