import { writable, get } from 'svelte/store';
import { toast } from "svelte-sonner";

type BackupExportState = {
    isExporting: boolean;
    progress: number;  // 0-100
    downloaded: number;  // 已下载字节
    total: number;  // 总字节 (0 表示未知)
    error: string | null;
};

const initialState: BackupExportState = {
    isExporting: false,
    progress: 0,
    downloaded: 0,
    total: 0,
    error: null
};

function createBackupExportStore() {
    const { subscribe, set, update } = writable<BackupExportState>(initialState);

    return {
        subscribe,

        reset: () => set(initialState),

        setProgress: (downloaded: number, total: number, percent: number) => {
            update(s => ({
                ...s,
                downloaded,
                total,
                progress: percent
            }));
        },

        startExport: () => {
            update(s => ({
                ...s,
                isExporting: true,
                progress: 0,
                downloaded: 0,
                total: 0,
                error: null
            }));
        },

        completeExport: () => {
            update(s => ({
                ...s,
                isExporting: false,
                progress: 100
            }));

            // 3秒后重置状态
            setTimeout(() => {
                update(s => {
                    // 只有在没有新导出开始时才重置
                    if (!s.isExporting && s.progress === 100) {
                        return initialState;
                    }
                    return s;
                });
            }, 3000);
        },

        failExport: (error: string) => {
            update(s => ({
                ...s,
                isExporting: false,
                error
            }));
            toast.error("备份失败", { description: error });
        },

        // 获取当前状态（用于检查是否正在导出）
        getState: () => get({ subscribe })
    };
}

export const backupExport = createBackupExportStore();
