async function apiQuery(params, signal) {
  const res = await fetch('/api/query?' + params.toString(), { signal });
  if (!res.ok) throw new Error('query failed: ' + res.status);
  return res.json();
}

async function apiUploadFile(file) {
  const res = await fetch('/api/upload?name=' + encodeURIComponent(file.name), {
    method: 'POST',
    body: file,
  });
  if (!res.ok) throw new Error('upload failed: ' + res.status + ' ' + res.statusText);
  return res.json();
}
