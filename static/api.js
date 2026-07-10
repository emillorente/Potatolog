async function apiQuery(params) {
  const res = await fetch('/api/query?' + params.toString());
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
