export const dynamic = "force-static";
export const dynamicParams = false;

export async function GET() {
  return Response.json({ ok: true, links: [] });
}