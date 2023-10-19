import { getLatestRelease, githubFetch } from "~/app/api/github";

export async function LatestVersion() {
    const release = await githubFetch(getLatestRelease);
    
    return (<>Alpha v{ release.tag_name }</>)
}