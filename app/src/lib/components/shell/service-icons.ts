export interface ServiceIcon {
    key: string;
    tile: string;
    svg: string;
}

export const SERVICE_ICONS: ServiceIcon[] = [
    {
        key: "virtual-machines",
        tile: "radial-gradient(circle at 100% 0%, var(--stage-olive) 0%, transparent 72%), var(--bg-inset)",
        svg: "<rect x=\"6\" y=\"6\" width=\"12\" height=\"12\" rx=\"2\"></rect><rect x=\"9.5\" y=\"9.5\" width=\"5\" height=\"5\" rx=\"1\"></rect><path d=\"M9 3v3M15 3v3M9 18v3M15 18v3M3 9h3M3 15h3M18 9h3M18 15h3\"></path>",
    },
    {
        key: "kubernetes",
        tile: "radial-gradient(circle at 100% 0%, var(--stage-olive) 0%, transparent 72%), var(--bg-inset)",
        svg: "<path d=\"M12 2.5l8.5 4.75v9.5L12 21.5l-8.5-4.75v-9.5z\"></path><path d=\"M12 12.5V8.5M12 12.5l-3.4 2.2M12 12.5l3.4 2.2\"></path><circle cx=\"12\" cy=\"7.5\" r=\"1.25\" fill=\"currentColor\"></circle><circle cx=\"7.8\" cy=\"15.2\" r=\"1.25\" fill=\"currentColor\"></circle><circle cx=\"16.2\" cy=\"15.2\" r=\"1.25\" fill=\"currentColor\"></circle>",
    },
    {
        key: "functions",
        tile: "radial-gradient(circle at 100% 0%, var(--stage-olive) 0%, transparent 72%), var(--bg-inset)",
        svg: "<path d=\"M8 4c-1.6 0-2 .8-2 2.2V10c0 1.1-.9 2-2 2 1.1 0 2 .9 2 2v3.8c0 1.4.4 2.2 2 2.2M16 4c1.6 0 2 .8 2 2.2V10c0 1.1.9 2 2 2-1.1 0-2 .9-2 2v3.8c0 1.4-.4 2.2-2 2.2\"></path><path d=\"M13 7.5l-3 4.75h4l-3 4.25\"></path>",
    },
    {
        key: "docker-containers",
        tile: "radial-gradient(circle at 100% 0%, var(--stage-olive) 0%, transparent 65%), radial-gradient(circle at 0% 100%, var(--stage-ember) 0%, transparent 65%), var(--bg-inset)",
        svg: "<rect x=\"3\" y=\"6.5\" width=\"18\" height=\"11\" rx=\"1.5\"></rect><path d=\"M7 9.5v5M10.5 9.5v5M14 9.5v5M17.5 9.5v5\"></path>",
    },
    {
        key: "docker-compose",
        tile: "radial-gradient(circle at 100% 0%, var(--stage-olive) 0%, transparent 65%), radial-gradient(circle at 0% 100%, var(--stage-ember) 0%, transparent 65%), var(--bg-inset)",
        svg: "<rect x=\"3\" y=\"3.5\" width=\"11\" height=\"7\" rx=\"1.25\"></rect><path d=\"M6 5.75v2.5M8.5 5.75v2.5M11 5.75v2.5\"></path><rect x=\"10\" y=\"13.5\" width=\"11\" height=\"7\" rx=\"1.25\"></rect><path d=\"M13 15.75v2.5M15.5 15.75v2.5M18 15.75v2.5\"></path><path d=\"M6 10.5v6.5h4\"></path>",
    },
    {
        key: "object-storage",
        tile: "radial-gradient(circle at 0% 100%, var(--stage-teal) 0%, transparent 72%), var(--bg-inset)",
        svg: "<ellipse cx=\"12\" cy=\"6\" rx=\"8\" ry=\"2.75\"></ellipse><path d=\"M4 6l1.8 12.2c.2 1.5 2.9 2.8 6.2 2.8s6-1.3 6.2-2.8L20 6\"></path><path d=\"M5 12.2c1.6 1 4.1 1.6 7 1.6s5.4-.6 7-1.6\"></path>",
    },
    {
        key: "block-storage",
        tile: "radial-gradient(circle at 0% 100%, var(--stage-teal) 0%, transparent 72%), var(--bg-inset)",
        svg: "<rect x=\"3.5\" y=\"3.5\" width=\"7.5\" height=\"7.5\" rx=\"1.5\"></rect><rect x=\"13\" y=\"3.5\" width=\"7.5\" height=\"7.5\" rx=\"1.5\"></rect><rect x=\"3.5\" y=\"13\" width=\"7.5\" height=\"7.5\" rx=\"1.5\"></rect><rect x=\"13\" y=\"13\" width=\"7.5\" height=\"7.5\" rx=\"1.5\" fill=\"currentColor\" fill-opacity=\"0.25\"></rect>",
    },
    {
        key: "file-storage",
        tile: "radial-gradient(circle at 0% 100%, var(--stage-teal) 0%, transparent 72%), var(--bg-inset)",
        svg: "<path d=\"M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z\"></path><path d=\"M8 12h8M8 15.5h5\"></path>",
    },
    {
        key: "virtual-network",
        tile: "radial-gradient(circle at 0% 0%, var(--stage-slate) 0%, transparent 72%), var(--bg-inset)",
        svg: "<rect x=\"3\" y=\"3\" width=\"18\" height=\"18\" rx=\"4\" stroke-dasharray=\"2.5 2.5\"></rect><circle cx=\"8\" cy=\"9\" r=\"1.75\"></circle><circle cx=\"16\" cy=\"9\" r=\"1.75\"></circle><circle cx=\"12\" cy=\"16\" r=\"1.75\"></circle><path d=\"M9.75 9h4.5M8.9 10.5l2.2 4M15.1 10.5l-2.2 4\"></path>",
    },
    {
        key: "dns",
        tile: "radial-gradient(circle at 0% 0%, var(--stage-slate) 0%, transparent 72%), var(--bg-inset)",
        svg: "<path d=\"M12 3v18M9 21h6\"></path><path d=\"M12 5h6.5l2 2.5-2 2.5H12\"></path><path d=\"M12 12H5.5l-2 2.5 2 2.5H12\"></path>",
    },
    {
        key: "load-balancer",
        tile: "radial-gradient(circle at 0% 0%, var(--stage-slate) 0%, transparent 72%), var(--bg-inset)",
        svg: "<circle cx=\"12\" cy=\"5\" r=\"2\"></circle><path d=\"M12 7v4M12 11l-7 6M12 11v6M12 11l7 6\"></path><rect x=\"3\" y=\"17\" width=\"4\" height=\"4\" rx=\"1\"></rect><rect x=\"10\" y=\"17\" width=\"4\" height=\"4\" rx=\"1\"></rect><rect x=\"17\" y=\"17\" width=\"4\" height=\"4\" rx=\"1\"></rect>",
    },
    {
        key: "web-app-firewall",
        tile: "radial-gradient(circle at 0% 0%, var(--stage-slate) 0%, transparent 72%), var(--bg-inset)",
        svg: "<path d=\"M12 3l7.5 3v5.5c0 4.5-3 8-7.5 9.5-4.5-1.5-7.5-5-7.5-9.5V6z\"></path><path d=\"M7 10h10M7.6 14h8.8M12 10v4M9.5 14v3.6M14.5 14v3.6M9.5 6.6V10M14.5 6.6V10\"></path>",
    },
    {
        key: "sql-database",
        tile: "radial-gradient(circle at 50% 110%, var(--stage-ember) 0%, transparent 70%), var(--bg-inset)",
        svg: "<ellipse cx=\"12\" cy=\"5.5\" rx=\"7.5\" ry=\"2.5\"></ellipse><path d=\"M4.5 5.5v13c0 1.4 3.4 2.5 7.5 2.5s7.5-1.1 7.5-2.5v-13\"></path><path d=\"M4.5 12c0 1.4 3.4 2.5 7.5 2.5s7.5-1.1 7.5-2.5\"></path>",
    },
    {
        key: "nosql-database",
        tile: "radial-gradient(circle at 50% 110%, var(--stage-ember) 0%, transparent 70%), var(--bg-inset)",
        svg: "<path d=\"M14 3H6.5A1.5 1.5 0 0 0 5 4.5v15A1.5 1.5 0 0 0 6.5 21h11a1.5 1.5 0 0 0 1.5-1.5V8z\"></path><path d=\"M14 3v5h5\"></path><path d=\"M10 11.5c-1 0-1.5.5-1.5 1.25v.75c0 .55-.45 1-1 1 .55 0 1 .45 1 1v.75c0 .75.5 1.25 1.5 1.25M14 11.5c1 0 1.5.5 1.5 1.25v.75c0 .55.45 1 1 1-.55 0-1 .45-1 1v.75c0 .75-.5 1.25-1.5 1.25\"></path>",
    },
    {
        key: "data-warehouse",
        tile: "radial-gradient(circle at 50% 110%, var(--stage-ember) 0%, transparent 70%), var(--bg-inset)",
        svg: "<path d=\"M3 10l9-6 9 6v10H3z\"></path><path d=\"M7.5 20v-7.5h9V20M7.5 15h9M7.5 17.5h9\"></path>",
    },
    {
        key: "big-data",
        tile: "radial-gradient(circle at 0% 100%, var(--stage-teal) 0%, transparent 65%), radial-gradient(circle at 100% 0%, var(--stage-slate) 0%, transparent 65%), var(--bg-inset)",
        svg: "<path d=\"M12 3l9 4.5-9 4.5-9-4.5z\"></path><path d=\"M3 12l9 4.5 9-4.5\"></path><path d=\"M3 16.5l9 4.5 9-4.5\"></path>",
    },
    {
        key: "streaming",
        tile: "radial-gradient(circle at 0% 100%, var(--stage-teal) 0%, transparent 65%), radial-gradient(circle at 100% 0%, var(--stage-slate) 0%, transparent 65%), var(--bg-inset)",
        svg: "<path d=\"M3 7.5c3-3 6-3 9 0s6 3 9 0\"></path><path d=\"M3 12.5c3-3 6-3 9 0s6 3 9 0\"></path><path d=\"M3 17.5c3-3 6-3 9 0\"></path><path d=\"M16 17.5h5M18.5 15l2.5 2.5-2.5 2.5\"></path>",
    },
    {
        key: "machine-learning",
        tile: "radial-gradient(circle at 0% 100%, var(--stage-teal) 0%, transparent 65%), radial-gradient(circle at 100% 0%, var(--stage-slate) 0%, transparent 65%), var(--bg-inset)",
        svg: "<path d=\"M5 8l7-2M5 8l7 10M5 16l7-10M5 16l7 2M12 6l7 6M12 18l7-6\"></path><circle cx=\"5\" cy=\"8\" r=\"1.75\" fill=\"currentColor\"></circle><circle cx=\"5\" cy=\"16\" r=\"1.75\" fill=\"currentColor\"></circle><circle cx=\"12\" cy=\"6\" r=\"1.75\" fill=\"currentColor\"></circle><circle cx=\"12\" cy=\"18\" r=\"1.75\" fill=\"currentColor\"></circle><circle cx=\"19\" cy=\"12\" r=\"1.75\" fill=\"currentColor\"></circle>",
    },
    {
        key: "data-integration",
        tile: "radial-gradient(circle at 0% 100%, var(--stage-teal) 0%, transparent 65%), radial-gradient(circle at 100% 0%, var(--stage-slate) 0%, transparent 65%), var(--bg-inset)",
        svg: "<circle cx=\"4.5\" cy=\"6\" r=\"1.5\"></circle><circle cx=\"4.5\" cy=\"18\" r=\"1.5\"></circle><path d=\"M6 6h3c3 0 3 6 6 6h5M6 18h3c3 0 3-6 6-6\"></path><path d=\"M17.5 9.5L20 12l-2.5 2.5\"></path>",
    },
    {
        key: "events",
        tile: "radial-gradient(circle at 100% 0%, var(--stage-olive) 0%, transparent 65%), radial-gradient(circle at 0% 100%, var(--stage-teal) 0%, transparent 65%), var(--bg-inset)",
        svg: "<circle cx=\"12\" cy=\"12\" r=\"1.75\" fill=\"currentColor\"></circle><path d=\"M8.5 8.5a5 5 0 0 0 0 7M15.5 8.5a5 5 0 0 1 0 7M5.6 5.6a9 9 0 0 0 0 12.8M18.4 5.6a9 9 0 0 1 0 12.8\"></path>",
    },
    {
        key: "queues",
        tile: "radial-gradient(circle at 100% 0%, var(--stage-olive) 0%, transparent 65%), radial-gradient(circle at 0% 100%, var(--stage-teal) 0%, transparent 65%), var(--bg-inset)",
        svg: "<rect x=\"3\" y=\"5\" width=\"4.5\" height=\"9\" rx=\"1\"></rect><rect x=\"9.75\" y=\"5\" width=\"4.5\" height=\"9\" rx=\"1\"></rect><rect x=\"16.5\" y=\"5\" width=\"4.5\" height=\"9\" rx=\"1\" fill=\"currentColor\" fill-opacity=\"0.25\"></rect><path d=\"M4 18.5h16M17.5 16l2.5 2.5-2.5 2.5\"></path>",
    },
    {
        key: "notifications",
        tile: "radial-gradient(circle at 100% 0%, var(--stage-olive) 0%, transparent 65%), radial-gradient(circle at 0% 100%, var(--stage-teal) 0%, transparent 65%), var(--bg-inset)",
        svg: "<path d=\"M6 16v-5a6 6 0 0 1 9-5.2M18 11v5l1.5 2h-15L6 16\"></path><path d=\"M10 21a2 2 0 0 0 4 0\"></path><circle cx=\"18.5\" cy=\"5.5\" r=\"2\" fill=\"currentColor\"></circle>",
    },
    {
        key: "monitoring",
        tile: "radial-gradient(circle at 0% 0%, var(--stage-slate) 0%, transparent 65%), radial-gradient(circle at 100% 100%, var(--stage-olive) 0%, transparent 65%), var(--bg-inset)",
        svg: "<rect x=\"3\" y=\"4\" width=\"18\" height=\"16\" rx=\"2\"></rect><path d=\"M6.5 13.5h2.5l2-5 2.5 8.5 2-3.5h2\"></path>",
    },
    {
        key: "infrastructure-as-code",
        tile: "radial-gradient(circle at 0% 0%, var(--stage-slate) 0%, transparent 65%), radial-gradient(circle at 100% 100%, var(--stage-olive) 0%, transparent 65%), var(--bg-inset)",
        svg: "<path d=\"M8 7l-4.5 5L8 17M16 7l4.5 5L16 17M13.5 5l-3 14\"></path>",
    },
    {
        key: "identity-access",
        tile: "radial-gradient(circle at 0% 100%, var(--stage-ember) 0%, transparent 65%), radial-gradient(circle at 100% 0%, var(--stage-slate) 0%, transparent 65%), var(--bg-inset)",
        svg: "<circle cx=\"10\" cy=\"8\" r=\"3.5\"></circle><path d=\"M3.5 20c0-3.6 2.9-6 6.5-6 1.5 0 2.9.4 4 1.1\"></path><circle cx=\"18\" cy=\"13\" r=\"2\"></circle><path d=\"M18 15v6M18 18h2M18 20.5h1.5\"></path>",
    },
    {
        key: "key-vault",
        tile: "radial-gradient(circle at 0% 100%, var(--stage-ember) 0%, transparent 65%), radial-gradient(circle at 100% 0%, var(--stage-slate) 0%, transparent 65%), var(--bg-inset)",
        svg: "<rect x=\"3\" y=\"4\" width=\"18\" height=\"15\" rx=\"2\"></rect><circle cx=\"13\" cy=\"11.5\" r=\"3.5\"></circle><path d=\"M13 9.5v4M11 11.5h4M6.5 8v7M6 19v2M18 19v2\"></path>",
    },
];

const BY_KEY = new Map(SERVICE_ICONS.map((icon) => [icon.key, icon]));

export function serviceIcon(key: string): ServiceIcon {
    const icon = BY_KEY.get(key);
    if (!icon) throw new Error(`unknown service icon ${key}`);
    return icon;
}
