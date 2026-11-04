<script lang="ts">
    import { ExternalLink, Rocket, Trash2 } from "@lucide/svelte";
    import { goto } from "$app/navigation";
    import type { SiteSummary } from "$lib/api/sites";
    import ActionMenu from "$lib/components/resource-group/action-menu.svelte";
    import type { MenuAction } from "$lib/components/resource-group/menu-action";
    import ServiceIcon from "$lib/components/shell/service-icon.svelte";
    import { badgeClass, deploymentStatus, repoLabel } from "$lib/utils/site-status";

    let { s, href, onRedeploy, onDelete }: { s: SiteSummary; href: string; onRedeploy: () => void; onDelete: () => void } = $props();

    const status = $derived(deploymentStatus(s.latest?.status));
    const domain = $derived(s.domains[0]);
    const actions = $derived<MenuAction[]>([
        { label: "Open", icon: ExternalLink, onselect: () => goto(href) },
        { label: "Redeploy", icon: Rocket, onselect: onRedeploy },
        { label: "Delete", icon: Trash2, onselect: onDelete, danger: true },
    ]);
</script>

<tr>
    <td>
        <div class="rs-name">
            <ServiceIcon name="web-apps" />
            <span class="rs-text">
                <button class="rs-link" type="button" onclick={() => goto(href)}>{s.name}</button>
                <span>{s.mode === "static" ? "Static site" : "Container app"}</span>
            </span>
        </div>
    </td>
    <td><span class={badgeClass(status.tone)}>{status.label}</span></td>
    <td class="cx-soft">{domain ?? repoLabel(s.repo_url)}</td>
    <td class="cx-soft">&mdash;</td>
    <td class="cx-num">{s.created_at.slice(0, 10)}</td>
    <td><ActionMenu label="Actions for {s.name}" {actions} /></td>
</tr>
