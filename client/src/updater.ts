export interface UpdateInfo {
  version: string;
  changelog: string;
  githubUrl: string;
  quarkUrl: string;
}

const GITHUB_REPO = 'donniemarc/plotvault-PDM';
const GITHUB_API = `https://api.github.com/repos/${GITHUB_REPO}/releases/latest`;
const CURRENT_VERSION = '2.0.3';

export async function checkForUpdates(): Promise<UpdateInfo | null> {
  try {
    const response = await fetch(GITHUB_API, {
      headers: { 'Accept': 'application/vnd.github.v3+json' }
    });
    if (!response.ok) return null;
    
    const release = await response.json();
    const latestVersion = release.tag_name.replace('v', '');
    
    if (compareVersions(latestVersion, CURRENT_VERSION) > 0) {
      const { githubUrl, quarkUrl } = parseDownloadUrls(release);
      
      return {
        version: latestVersion,
        changelog: release.body || '暂无更新日志',
        githubUrl,
        quarkUrl
      };
    }
  } catch (error) {
    console.error('检查更新失败:', error);
  }
  return null;
}

function compareVersions(a: string, b: string): number {
  const pa = a.split('.').map(Number);
  const pb = b.split('.').map(Number);
  for (let i = 0; i < 3; i++) {
    if (pa[i] > pb[i]) return 1;
    if (pa[i] < pb[i]) return -1;
  }
  return 0;
}

function parseDownloadUrls(release: any): { githubUrl: string; quarkUrl: string } {
  const body = release.body || '';
  
  const quarkMatch = body.match(/https:\/\/pan\.quark\.cn\/[^\s)]+/);
  const quarkUrl = quarkMatch ? quarkMatch[0] : '';
  
  const githubUrl = release.assets?.[0]?.browser_download_url ||
    `https://github.com/donniemarc/plotvault-PDM/releases/download/${release.tag_name}/PlotVault.PDM_${release.tag_name}_x64-setup.exe`;
  
  return { githubUrl, quarkUrl };
}

export function getDownloadUrl(update: UpdateInfo): string {
  return update.githubUrl || update.quarkUrl;
}

export function getCurrentVersion(): string {
  return CURRENT_VERSION;
}
