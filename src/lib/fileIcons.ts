import {
  FileCode2, FileJson, FileText, FileImage, FileSpreadsheet, FileCog,
  File as FileIcon, type LucideIcon,
} from 'lucide-react';

const CODE_EXTENSIONS = new Set([
  'ts', 'tsx', 'js', 'jsx', 'rs', 'py', 'go', 'rb', 'c', 'cpp', 'h', 'hpp',
  'java', 'kt', 'swift', 'sh', 'html', 'css', 'scss', 'sql',
]);
const IMAGE_EXTENSIONS = new Set(['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'bmp', 'ico']);
const DATA_EXTENSIONS = new Set(['csv', 'xlsx', 'tsv']);
const CONFIG_EXTENSIONS = new Set(['toml', 'yaml', 'yml', 'ini', 'env', 'lock']);

export function fileTypeIcon(name: string): { Icon: LucideIcon; color: string } {
  const ext = name.split('.').pop()?.toLowerCase() ?? '';
  if (ext === 'json') return { Icon: FileJson, color: 'var(--text-muted)' };
  if (CODE_EXTENSIONS.has(ext)) return { Icon: FileCode2, color: 'var(--accent)' };
  if (IMAGE_EXTENSIONS.has(ext)) return { Icon: FileImage, color: '#22c55e' };
  if (DATA_EXTENSIONS.has(ext)) return { Icon: FileSpreadsheet, color: '#22c55e' };
  if (CONFIG_EXTENSIONS.has(ext)) return { Icon: FileCog, color: 'var(--text-muted)' };
  if (ext === 'md' || ext === 'txt') return { Icon: FileText, color: 'var(--text-muted)' };
  return { Icon: FileIcon, color: 'var(--text-muted)' };
}
