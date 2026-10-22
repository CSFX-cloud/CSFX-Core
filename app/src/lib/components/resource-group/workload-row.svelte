<script lang="ts">
    import { RefreshCw, Square, Trash2 } from "@lucide/svelte";
    import type { Workload } from "$lib/api/resource-groups";
    import ActionMenu from "$lib/components/resource-group/action-menu.svelte";
    import type { MenuAction } from "$lib/components/resource-group/menu-action";
    import { formatBytes } from "$lib/utils/format";
    import ServiceIcon from "$lib/components/shell/service-icon.svelte";
    import { knownWorkloadIcon } from "$lib/utils/image-icon";
    import { badgeClass, statusLabel } from "$lib/utils/tone";

    const MILLICORES_PER_CORE = 1000;

    type Props = {
        w: Workload;
        href: string;
        icon: string;
        nodeName: string;
        indented?: boolean;
        onRestart: () => void;
        onStop: () => void;
        onDelete: () => void;
    };

    let { w, href, icon, nodeName, indented = false, onRestart, onStop, onDelete }: Props = $props();

    const isVm = $derived(w.runtime_class === "vm");
    const details = $derived(isVm ? `${w.cpu_millicores / MILLICORES_PER_CORE} vCPU · ${formatBytes(w.memory_bytes)}` : containerDetails(w));
    const actions = $derived<MenuAction[]>([
        { label: "Restart", icon: RefreshCw, onselect: onRestart },
        { label: "Stop", icon: Square, onselect: onStop, disabled: w.desired_state === "stopped" },
        { label: "Delete", icon: Trash2, onselect: onDelete, danger: true },
    ]);

    function containerDetails(workload: Workload): string {
        const port = workload.ports?.[0]?.container_port;
        return port ? `${workload.image} · port ${port}` : workload.image;
    }
</script>

<tr>
    <td>
        <div class="rs-name {indented ? 'rs-name--child' : ''}">
            <ServiceIcon name={icon} brand={knownWorkloadIcon(w.image, w.runtime_class)} />
            <a class="rs-link" {href}>{w.service_name ?? w.name}</a>
        </div>
    </td>
    <td>
        <span class={badgeClass(w.status)}>{statusLabel(w.status)}</span>
        {#if w.restart_count > 0}
            <span class="rs-restarts" title="Restarted {w.restart_count} time{w.restart_count === 1 ? '' : 's'}{w.max_restarts !== null ? ` (max ${w.max_restarts})` : ''}">
                &#8635; {w.restart_count}
            </span>
        {/if}
    </td>
    <td class="cx-soft">{details}</td>
    <td class="cx-soft">{nodeName}</td>
    <td class="cx-num">{w.created_at.slice(0, 10)}</td>
    <td><ActionMenu label="Actions for {w.service_name ?? w.name}" {actions} /></td>
</tr>
