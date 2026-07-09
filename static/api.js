/** API bridge: local HTTP server in desktop, fetch in browser mode. */

function useHttpApi() {
  return window.location.protocol === 'http:' || window.location.protocol === 'https:';
}

async function apiQuery(params) {
  if (useHttpApi()) {
    const res = await fetch('/api/query?' + params.toString());
    if (!res.ok) throw new Error('query failed: ' + res.status);
    return res.json();
  }
  throw new Error('HTTP API not available');
}

async function apiUploadFile(file) {
  const res = await fetch('/api/upload?name=' + encodeURIComponent(file.name), {
    method: 'POST',
    body: file,
  });
  if (!res.ok) throw new Error('upload failed: ' + res.status + ' ' + res.statusText);
  return res.json();
}

async function apiPickLogFile() {
  const res = await fetch('/api/open');
  if (!res.ok) throw new Error('open failed: ' + res.status);
  const data = await res.json();
  if (!data || !data.path) return null;
  return data;
}
