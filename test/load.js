import http from 'k6/http';
import { check, sleep } from 'k6';

export const options = {
  vus: 5,
  duration: '10s',
};

export default function () {
  const res = http.get('http://localhost:3000', {
    headers: {
      "X-App": "some-app"
    }
  });
  console.log(res.status);
  check(res, { 'status was 200': (r) => r.status == 200 });
}