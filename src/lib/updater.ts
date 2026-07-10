import { errorMessage } from './tauri';

export type UpdateCheckResult =
  | { status: 'up-to-date' }
  | { status: 'installed' }
  | { status: 'declined' }
  | { status: 'error'; message: string };

export type UpdateCheckProgress = 'checking' | 'available' | 'downloading';

/**
 * Checks for an app update, and if one exists, confirms with the user,
 * downloads/installs it, and relaunches — the single code path shared by
 * the manual "Check for updates" button in Settings and the automatic
 * startup check, so both surfaces behave identically. `onProgress` is
 * optional so a silent background caller (the startup check) doesn't need
 * to care about intermediate states, while Settings can still show them.
 */
export async function checkForUpdates(onProgress?: (progress: UpdateCheckProgress) => void): Promise<UpdateCheckResult> {
  try {
    onProgress?.('checking');
    const { check } = await import('@tauri-apps/plugin-updater');
    const update = await check();
    if (!update) return { status: 'up-to-date' };

    onProgress?.('available');
    const { confirm: confirmDialog } = await import('@tauri-apps/plugin-dialog');
    const proceed = await confirmDialog(`Version ${update.version} is available.`, { title: 'Download and install?' });
    if (!proceed) return { status: 'declined' };

    onProgress?.('downloading');
    await update.downloadAndInstall();
    const { relaunch } = await import('@tauri-apps/plugin-process');
    await relaunch();
    return { status: 'installed' };
  } catch (e) {
    return { status: 'error', message: errorMessage(e) };
  }
}
