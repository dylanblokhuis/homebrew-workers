async function onRequest(request) {
  const response = new Response("Test", {
    headers: {
      "X-Proto": "Test"
    }
  });

  respondWith({
    headers: Object.fromEntries(response.headers),
    ok: response.ok,
    redirected: response.redirected,
    status: response.status,
    statusText: response.statusText,
    trailer: response.trailer,
    type: response.type,
    body: await response.text()
  })
}