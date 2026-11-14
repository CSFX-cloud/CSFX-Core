import { api } from './http';

export interface ClusterVip {
    address: string;
    prefix_length: number;
    router_id: number;
}

export const getClusterVip = async (): Promise<ClusterVip | null> =>
    (await api<{ vip: ClusterVip | null }>('GET', '/cluster/vip')).vip;

export const updateClusterVip = async (vip: ClusterVip): Promise<ClusterVip> =>
    (await api<{ vip: ClusterVip }>('PUT', '/cluster/vip', vip)).vip;
