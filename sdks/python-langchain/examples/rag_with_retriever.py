"""Example: RAG Chain with Adaptive Memory Retriever.

This demo shows how to use AdaptiveMemoryRetriever in a LangChain RAG pipeline
to retrieve context from Aetheris MemOS long-term memory.

Requirements:
    pip install adaptive-memory adaptive-memory-langchain langchain-openai

Usage:
    export AETHERIS_BASE_URL=http://localhost:8008
    export AETHERIS_TOKEN=your-token
    export OPENAI_API_KEY=your-openai-key
    python rag_with_retriever.py
"""

import os

from adaptive_memory import MemoryClient
from adaptive_memory_langchain import AdaptiveMemoryRetriever

# Initialize memory client
client = MemoryClient(
    base_url=os.getenv("AETHERIS_BASE_URL", "http://localhost:8008"),
    api_key=os.getenv("AETHERIS_TOKEN"),
)

# Create the retriever
retriever = AdaptiveMemoryRetriever(
    client=client,
    search_type="hybrid",  # Options: hybrid, ltm, triple, scored
    top_k=5,
    min_score=0.3,
    user_id="demo-user",
)

print("AdaptiveMemoryRetriever configured:")
print(f"  search_type: {retriever.search_type}")
print(f"  top_k: {retriever.top_k}")
print(f"  min_score: {retriever.min_score}")

# --- Direct retriever usage ---
print("\n--- Retrieving documents ---")
docs = retriever.invoke("adaptive memory systems")
print(f"  Found {len(docs)} documents")
for i, doc in enumerate(docs):
    print(f"  [{i}] score={doc.metadata.get('score', 0):.2f} "
          f"layer={doc.metadata.get('source_layer', '?')} "
          f"content={doc.page_content[:80]}...")

# --- RAG Chain (uncomment with OpenAI key) ---
#
# from langchain_openai import ChatOpenAI
# from langchain_core.prompts import ChatPromptTemplate
# from langchain_core.output_parsers import StrOutputParser
# from langchain_core.runnables import RunnablePassthrough
#
# llm = ChatOpenAI(model="gpt-4o-mini", temperature=0)
#
# prompt = ChatPromptTemplate.from_template(
#     "Answer the question based on the following context from memory:\n\n"
#     "Context:\n{context}\n\n"
#     "Question: {question}\n\n"
#     "Answer:"
# )
#
# def format_docs(docs):
#     return "\n\n".join(doc.page_content for doc in docs)
#
# rag_chain = (
#     {"context": retriever | format_docs, "question": RunnablePassthrough()}
#     | prompt
#     | llm
#     | StrOutputParser()
# )
#
# answer = rag_chain.invoke("How does adaptive memory routing work?")
# print(f"\nRAG Answer: {answer}")
