// import Link from 'next/link';
import Layout from '../components/Layout';
import { Bubbles } from '../components/Bubbles';
import { Button } from '@sd/ui';

const IndexPage = () => (
  <Layout title="Spacedrive: The file explorer from the future.">
    <img src="app-logo.svg" className="w-40 mt-20" />
    <h1 className="mt-10 text-6xl font-black">The file explorer from the future</h1>
    <p className="mt-1 mb-10 text-lg text-gray-450">
      Spacedrive is the first file manager that puts the full power of the cloud in your hands.
    </p>
    <Button variant="primary" className="mb-10">
      Download
    </Button>

    <iframe
      className="z-50 border rounded-lg shadow-2xl border-gray-550"
      width={1200}
      height={600}
      src="http://localhost:8002?library_id=9068c6ec-cf90-451b-bb30-4174781e7bc6"
    />
    <Bubbles />
  </Layout>
);

export default IndexPage;
