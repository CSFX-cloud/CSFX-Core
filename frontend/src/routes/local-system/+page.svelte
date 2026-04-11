<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    getLocalSystemInfo,
    getLocalSystemMetrics,
    formatUptime,
    type LocalSystemInfo,
    type LocalSystemMetrics,
  } from '$lib/services/system';
  import { formatBytes } from '$lib/services/agents';
  import * as Card from '$lib/components/ui/card/index.js';
  import * as Chart from '$lib/components/ui/chart/index.js';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import { PieChart, Text } from 'layerchart';
  import { RefreshCw, Server, Cpu, HardDrive, Activity } from '@lucide/svelte';
  import Icon from '@iconify/svelte';

  let systemInfo = $state<LocalSystemInfo | null>(null);
  let currentMetrics = $state<LocalSystemMetrics | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let refreshInterval: ReturnType<typeof setInterval>;

  async function loadData() {
    try {
      const [info, metrics] = await Promise.all([getLocalSystemInfo(), getLocalSystemMetrics()]);
      systemInfo = info;
      currentMetrics = metrics;
      loading = false;
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load system data';
      loading = false;
    }
  }

  onMount(() => {
    loadData();
    refreshInterval = setInterval(loadData, 5000); // Refresh every 5 seconds
  });

  onDestroy(() => {
    if (refreshInterval) {
      clearInterval(refreshInterval);
    }
  });

  function getOsIconName(osName: string): string {
    const lower = osName?.toLowerCase() || '';
    if (lower.includes('mac')) return 'bi:apple';
    if (lower.includes('linux') || lower.includes('ubuntu')) return 'bi:ubuntu';
    if (lower.includes('windows')) return 'bi:windows';
    return 'bi:pc-display';
  }

  // Chart configurations
  const cpuRadialConfig = {
    used: { label: 'Used', color: 'oklch(0.646 0.222 41.116)' },
    free: { label: 'Free', color: 'oklch(0.5 0.002 286.375 / 0.3)' },
  } satisfies Chart.ChartConfig;

  const memoryRadialConfig = {
    used: { label: 'Used', color: 'oklch(0.6 0.118 184.704)' },
    free: { label: 'Free', color: 'oklch(0.5 0.002 286.375 / 0.3)' },
  } satisfies Chart.ChartConfig;

  const diskRadialConfig = {
    used: { label: 'Used', color: 'oklch(0.398 0.07 227.392)' },
    free: { label: 'Free', color: 'oklch(0.5 0.002 286.375 / 0.3)' },
  } satisfies Chart.ChartConfig;

  function getUsageColor(percentage: number): string {
    if (percentage < 80) return 'oklch(0.646 0.222 41.116)';
    else if (percentage < 90) return 'oklch(0.828 0.189 84.429)';
    else return 'oklch(0.577 0.245 27.325)';
  }

  const freeColor = 'oklch(0.5 0.002 286.375 / 0.3)';
</script>

<svelte:head>
  <title>Local System - CSFX Core</title>
</svelte:head>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <div>
      <h1 class="text-3xl font-bold">Local System</h1>
      <p class="text-muted-foreground">Monitor the system running the CSFX Core backend daemon</p>
    </div>
    <Button variant="outline" size="sm" onclick={loadData} disabled={loading}>
      <RefreshCw class="h-4 w-4 {loading ? 'animate-spin' : ''}" />
    </Button>
  </div>

  {#if loading && !systemInfo}
    <div class="flex h-64 items-center justify-center">
      <RefreshCw class="h-8 w-8 animate-spin text-muted-foreground" />
    </div>
  {:else if error}
    <div class="rounded-lg border border-destructive bg-destructive/10 p-4 text-destructive">
      {error}
    </div>
  {:else if systemInfo && currentMetrics}
    <!-- System Info Card -->
    <Card.Root>
      <Card.Header>
        <div class="flex items-start justify-between">
          <div class="flex items-center gap-4">
            <Icon
              icon={getOsIconName(systemInfo.os_name)}
              class="h-16 w-16 text-muted-foreground"
            />
            <div>
              <Card.Title class="text-3xl">{systemInfo.hostname}</Card.Title>
              <Card.Description class="mt-1 text-base">
                {systemInfo.os_name}
                {systemInfo.os_version}
              </Card.Description>
              <p class="mt-2 text-sm text-muted-foreground">
                Kernel: {systemInfo.kernel_version}
              </p>
            </div>
          </div>
          <Badge class="bg-green-500 hover:bg-green-600 text-white">
            <Activity class="mr-1 h-3 w-3" />
            Running
          </Badge>
        </div>
      </Card.Header>
      <Card.Content>
        <div class="grid grid-cols-2 gap-4 md:grid-cols-4">
          <div>
            <p class="text-sm text-muted-foreground">CPU Model</p>
            <p class="mt-1 text-sm font-medium">{systemInfo.cpu_model}</p>
          </div>
          <div>
            <p class="text-sm text-muted-foreground">CPU Cores</p>
            <p class="mt-1 text-sm font-medium">
              {systemInfo.cpu_cores} cores / {systemInfo.cpu_threads} threads
            </p>
          </div>
          <div>
            <p class="text-sm text-muted-foreground">Uptime</p>
            <p class="mt-1 text-sm font-medium">
              {formatUptime(systemInfo.uptime_seconds)}
            </p>
          </div>
          <div>
            <p class="text-sm text-muted-foreground">Last Update</p>
            <p class="mt-1 text-sm font-medium">
              {new Date(currentMetrics.timestamp).toLocaleTimeString('de-DE')}
            </p>
          </div>
        </div>
      </Card.Content>
    </Card.Root>

    <!-- Current Metrics - 3 Radial Charts -->
    <div class="grid grid-cols-1 gap-6 md:grid-cols-3">
      <!-- CPU Usage -->
      <Card.Root class="flex flex-col">
        <Card.Header class="items-center pb-0">
          <Card.Title>CPU Usage</Card.Title>
          <Card.Description>
            {currentMetrics.cpu_usage_percent.toFixed(1)}% used
          </Card.Description>
        </Card.Header>
        <Card.Content class="flex-1 pb-0">
          <Chart.Container config={cpuRadialConfig} class="mx-auto aspect-square max-h-[250px]">
            <PieChart
              data={[
                {
                  name: 'used',
                  value: currentMetrics.cpu_usage_percent,
                  color: getUsageColor(currentMetrics.cpu_usage_percent),
                },
                {
                  name: 'free',
                  value: 100 - currentMetrics.cpu_usage_percent,
                  color: freeColor,
                },
              ]}
              r={(d) => d.value}
              innerRadius={(radius) => radius * 0.67}
            >
              <Text
                value={currentMetrics.cpu_usage_percent}
                format={(d) => `${d.toFixed(1)}%`}
                class="text-3xl font-bold tabular-nums"
                y={-4}
              />
              <Text value="CPU" class="text-sm font-medium text-muted-foreground" y={16} />
            </PieChart>
          </Chart.Container>
        </Card.Content>
      </Card.Root>

      <!-- Memory Usage -->
      <Card.Root class="flex flex-col">
        <Card.Header class="items-center pb-0">
          <Card.Title>Memory Usage</Card.Title>
          <Card.Description>
            {formatBytes(currentMetrics.memory_used_bytes)} / {formatBytes(
              currentMetrics.memory_total_bytes
            )}
          </Card.Description>
        </Card.Header>
        <Card.Content class="flex-1 pb-0">
          <Chart.Container config={memoryRadialConfig} class="mx-auto aspect-square max-h-[250px]">
            <PieChart
              data={[
                {
                  name: 'used',
                  value: currentMetrics.memory_usage_percent,
                  color: getUsageColor(currentMetrics.memory_usage_percent),
                },
                {
                  name: 'free',
                  value: 100 - currentMetrics.memory_usage_percent,
                  color: freeColor,
                },
              ]}
              r={(d) => d.value}
              innerRadius={(radius) => radius * 0.67}
            >
              <Text
                value={currentMetrics.memory_usage_percent}
                format={(d) => `${d.toFixed(1)}%`}
                class="text-3xl font-bold tabular-nums"
                y={-4}
              />
              <Text value="Memory" class="text-sm font-medium text-muted-foreground" y={16} />
            </PieChart>
          </Chart.Container>
        </Card.Content>
      </Card.Root>

      <!-- Disk Usage -->
      <Card.Root class="flex flex-col">
        <Card.Header class="items-center pb-0">
          <Card.Title>Disk Usage</Card.Title>
          <Card.Description>
            {formatBytes(currentMetrics.disk_used_bytes)} / {formatBytes(
              currentMetrics.disk_total_bytes
            )}
          </Card.Description>
        </Card.Header>
        <Card.Content class="flex-1 pb-0">
          <Chart.Container config={diskRadialConfig} class="mx-auto aspect-square max-h-[250px]">
            <PieChart
              data={[
                {
                  name: 'used',
                  value: currentMetrics.disk_usage_percent,
                  color: getUsageColor(currentMetrics.disk_usage_percent),
                },
                {
                  name: 'free',
                  value: 100 - currentMetrics.disk_usage_percent,
                  color: freeColor,
                },
              ]}
              r={(d) => d.value}
              innerRadius={(radius) => radius * 0.67}
            >
              <Text
                value={currentMetrics.disk_usage_percent}
                format={(d) => `${d.toFixed(1)}%`}
                class="text-3xl font-bold tabular-nums"
                y={-4}
              />
              <Text value="Disk" class="text-sm font-medium text-muted-foreground" y={16} />
            </PieChart>
          </Chart.Container>
        </Card.Content>
      </Card.Root>
    </div>

    <!-- Network & Additional Info -->
    <div class="grid grid-cols-1 gap-6 md:grid-cols-2">
      <Card.Root>
        <Card.Header>
          <Card.Title>Network Statistics</Card.Title>
        </Card.Header>
        <Card.Content>
          <div class="space-y-4">
            <div class="flex justify-between">
              <span class="text-muted-foreground">Total Received:</span>
              <span class="font-medium">{formatBytes(currentMetrics.network_rx_bytes)}</span>
            </div>
            <div class="flex justify-between">
              <span class="text-muted-foreground">Total Transmitted:</span>
              <span class="font-medium">{formatBytes(currentMetrics.network_tx_bytes)}</span>
            </div>
          </div>
        </Card.Content>
      </Card.Root>

      <Card.Root>
        <Card.Header>
          <Card.Title>System Information</Card.Title>
        </Card.Header>
        <Card.Content>
          <div class="space-y-4">
            <div class="flex justify-between">
              <span class="text-muted-foreground">Architecture:</span>
              <span class="font-medium">x86_64</span>
            </div>
            <div class="flex justify-between">
              <span class="text-muted-foreground">Status:</span>
              <Badge class="bg-green-500 hover:bg-green-600 text-white">Healthy</Badge>
            </div>
          </div>
        </Card.Content>
      </Card.Root>
    </div>
  {/if}
</div>
