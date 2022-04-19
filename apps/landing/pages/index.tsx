import Link from 'next/link';
import Layout from '../components/Layout';

// import Spacedrive interface

const IndexPage = () => (
  <Layout title="Home | Next.js + TypeScript Example">
    <h1 className="my-8 text-4xl font-black">A file explorer from the future</h1>
    <iframe
      style={{ border: 'none', borderRadius: 5 }}
      width={1200}
      height={600}
      src="http://localhost:8002?library_id=9068c6ec-cf90-451b-bb30-4174781e7bc6"
    />
  </Layout>
);

export default IndexPage;
