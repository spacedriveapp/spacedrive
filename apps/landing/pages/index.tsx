import Link from 'next/link';
import Layout from '../components/Layout';

// import Spacedrive interface

const IndexPage = () => (
  <Layout title="Home | Next.js + TypeScript Example">
    <h1 className="my-16 text-6xl font-black">The file explorer from the future</h1>
    <iframe
      className="border border-gray-800 rounded-lg shadow-2xl"
      width={1200}
      height={600}
      src="http://localhost:8002?library_id=9068c6ec-cf90-451b-bb30-4174781e7bc6"
    />
  </Layout>
);

export default IndexPage;
