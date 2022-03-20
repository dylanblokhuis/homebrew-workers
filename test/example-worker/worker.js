async function onRequest(event) {
  const response = new Response("Hello from a worker!", {
    headers: {
      "X-Proto": "Test"
    }
  });

  event.respondWith(response)
}