async function fetchData(url: string): Promise<any> {
  try {
    const response = await fetch(url);
    const data = await response.json();
    console.log("Fetched data:", data);
    return data;
  } catch (err) {
    console.error("Error fetching data:", err);
  }
}
