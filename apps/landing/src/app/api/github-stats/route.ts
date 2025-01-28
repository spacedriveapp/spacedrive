import { NextResponse } from 'next/server';

const runtime = 'edge';
const revalidate = 21600; // 6 hours

async function getGitHubReleaseStats() {
  const response = await fetch(
    'https://api.github.com/repos/spacedriveapp/spacedrive/releases',
    {
      headers: {
        'Accept': 'application/vnd.github.v3+json',
        ...(process.env.GITHUB_TOKEN && {
          'Authorization': `token ${process.env.GITHUB_TOKEN}`
        })
      }
    }
  );

  if (!response.ok) {
    throw new Error('Failed to fetch GitHub stats');
  }

  const releases = await response.json();
  
  // Sum up download count from all assets across all releases
  const totalDownloads = releases.reduce((total: number, release: any) => {
    const releaseDownloads = release.assets.reduce((sum: number, asset: any) => {
      return sum + asset.download_count;
    }, 0);
    return total + releaseDownloads;
  }, 0);

  return totalDownloads;
}

export async function GET() {
  try {
    const downloads = await getGitHubReleaseStats();
    return NextResponse.json({ downloads });
  } catch (error) {
    console.error('Error fetching GitHub stats:', error);
    return NextResponse.json({ error: 'Failed to fetch download stats' }, { status: 500 });
  }
}
