'use client';

import { FormEvent, useEffect, useState } from 'react';

import { v4 as uuidv4 } from 'uuid';

interface ApiResponse {
    message: string;
    task_id?: string;
}

interface GenerateTextTaskPayload {
    task_id: string;
    prompt: string | null;
    max_length: number;
}

interface SharedGeneratedTextMessage {
    original_task_id: string;
    generated_text: string;
    timestamp_ms: number;
}

interface SemanticSearchApiRequestPayload {
    query_text: string;
    top_k: number;
}

interface QdrantPointPayload {
    original_document_id: string;
    source_url: string;
    sentence_text: string;
    sentence_order: number;
    model_name: string;
    processed_at_ms: number;
}

interface SemanticSearchResultItem {
    qdrant_point_id: string;
    score: number;
    payload: QdrantPointPayload;
}

interface SemanticSearchApiResponsePayload {
    search_request_id: string;
    results: SemanticSearchResultItem[];
    error_message: string | null;
}

export default function HomePage() {
    const [urlInput, setUrlInput] = useState<string>('');
    const [promptInput, setPromptInput] = useState<string>('');
    const [maxLengthInput, setMaxLengthInput] = useState<number>(50);
    const [statusMessage, setStatusMessage] = useState<string>('');
    const [generatedText, setGeneratedText] = useState<string>('... тут будет сгенерированный текст ...');
    // const [sseEventSource, setSseEventSource] = useState<EventSource | null>(null);

    const [searchQueryInput, setSearchQueryInput] = useState<string>('');
    const [searchTopKInput, setSearchTopKInput] = useState<number>(5);
    const [searchResults, setSearchResults] = useState<SemanticSearchResultItem[]>([]);
    const [searchStatusMessage, setSearchStatusMessage] = useState<string>('');

    const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:7070/api';

    useEffect(() => {
        const evtSource = new EventSource(`${API_BASE_URL}/events`);
        // setSseEventSource(evtSource);
        setStatusMessage('Подключено к потоку событий генерации...');
        console.log('SSE EventSource connection initiated.');

        evtSource.onmessage = (event) => {
            console.log('Raw SSE data received:', event.data);
            try {
                const parsedData: SharedGeneratedTextMessage = JSON.parse(event.data);
                console.log('Parsed SSE Data:', parsedData);
                setGeneratedText(parsedData.generated_text);
                setStatusMessage(`Текст (задача ${parsedData.original_task_id}) успешно сгенерирован!`);
            } catch (e) {
                console.error('Failed to parse SSE JSON:', e, 'Raw data:', event.data);
                setStatusMessage(`Ошибка парсинга SSE данных: ${event.data}`);
            }
        };

        evtSource.onerror = (err) => {
            console.error('EventSource failed:', err);
            setStatusMessage('Ошибка подключения к потоку событий. Попробуйте обновить страницу.');
            evtSource.close();
        };

        return () => {
            if (evtSource) {
                evtSource.close();
                console.log('SSE EventSource connection closed.');
            }
        };
    }, [API_BASE_URL]);

    const handleSubmitUrl = async (event: FormEvent) => {
        event.preventDefault();
        if (!urlInput.trim()) {
            setStatusMessage('URL не может быть пустым!');
            return;
        }
        setStatusMessage(`Отправка URL: ${urlInput}...`);
        try {
            const response = await fetch(`${API_BASE_URL}/submit-url`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ url: urlInput }),
            });
            const data: ApiResponse = await response.json();
            if (response.ok) {
                setStatusMessage(data.message);
            } else {
                setStatusMessage(`Ошибка сервера: ${data.message || response.statusText}`);
            }
        } catch (error) {
            console.error('Network error submitting URL:', error);
            setStatusMessage('Сетевая ошибка при отправке URL.');
        }
    };

    const handleGenerateText = async (event: FormEvent) => {
        event.preventDefault();
        const taskId = uuidv4();
        setStatusMessage(`Отправка задачи на генерацию текста (ID: ${taskId})...`);

        const payload: GenerateTextTaskPayload = {
            task_id: taskId,
            prompt: promptInput.trim() === '' ? null : promptInput.trim(),
            max_length: maxLengthInput > 0 ? maxLengthInput : 50,
        };

        try {
            const response = await fetch(`${API_BASE_URL}/generate-text`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(payload),
            });
            const data: ApiResponse = await response.json();
            if (response.ok) {
                setStatusMessage(data.message);
            } else {
                setStatusMessage(`Ошибка сервера при генерации: ${data.message || response.statusText}`);
            }
        } catch (error) {
            console.error('Network error generating text:', error);
            setStatusMessage('Сетевая ошибка при генерации текста.');
        }
    };

    const handleSemanticSearch = async (event: FormEvent) => {
        event.preventDefault();
        if (!searchQueryInput.trim()) {
            setSearchStatusMessage('Поисковый запрос не может быть пустым!');
            return;
        }
        setSearchStatusMessage(`Выполняется семантический поиск для: "${searchQueryInput}"...`);
        setSearchResults([]);

        const payload: SemanticSearchApiRequestPayload = {
            query_text: searchQueryInput.trim(),
            top_k: searchTopKInput > 0 ? searchTopKInput : 5,
        };

        try {
            const response = await fetch(`${API_BASE_URL}/search/semantic`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(payload),
            });

            const data: SemanticSearchApiResponsePayload = await response.json();

            if (response.ok) {
                if (data.error_message) {
                    setSearchStatusMessage(`Ошибка от сервера: ${data.error_message}`);
                    setSearchResults([]);
                } else {
                    setSearchResults(data.results);
                    setSearchStatusMessage(
                        `Найдено ${data.results.length} результатов для запроса ID: ${data.search_request_id}.`
                    );
                    if (data.results.length === 0) {
                        setSearchStatusMessage(`По вашему запросу ничего не найдено (ID: ${data.search_request_id}).`);
                    }
                }
            } else {
                setSearchStatusMessage(`Ошибка сервера при поиске: ${data.error_message || response.statusText}`);
                setSearchResults([]);
            }
        } catch (error) {
            console.error('Network error during semantic search:', error);
            setSearchStatusMessage('Сетевая ошибка при выполнении семантического поиска.');
            setSearchResults([]);
        }
    };

    return (
        <main className="flex min-h-screen flex-col items-center justify-between p-12 md:p-24 bg-gray-50 text-gray-800">
            <div className="z-10 w-full max-w-5xl items-center justify-between font-mono text-sm lg:flex flex-col space-y-8">
                <h1 className="text-4xl font-bold text-center text-indigo-700">Codename: Symbiont UI</h1>

                <div className="grid grid-cols-1 md:grid-cols-2 gap-8 w-full">
                    <div className="space-y-8">
                        <section className="w-full p-6 bg-white rounded-lg shadow-xl">
                            <h2 className="text-2xl font-semibold mb-4 text-gray-700 border-b pb-2">
                                1. Обработка URL
                            </h2>
                            <form onSubmit={handleSubmitUrl} className="space-y-4 mt-4">
                                <div>
                                    <label htmlFor="urlInput" className="block text-sm font-medium text-gray-600">
                                        URL для скрапинга:
                                    </label>
                                    <input
                                        id="urlInput"
                                        type="text"
                                        value={urlInput}
                                        onChange={(e) => setUrlInput(e.target.value)}
                                        placeholder="https://example.com"
                                        className="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm text-black"
                                    />
                                </div>
                                <button
                                    type="submit"
                                    className="w-full px-4 py-2 text-sm font-medium text-white bg-indigo-600 rounded-md hover:bg-indigo-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500 transition-colors duration-150"
                                >
                                    Отправить URL
                                </button>
                            </form>
                        </section>

                        <section className="w-full p-6 bg-white rounded-lg shadow-xl">
                            <h2 className="text-2xl font-semibold mb-4 text-gray-700 border-b pb-2">
                                2. Генерация Текста
                            </h2>
                            <form onSubmit={handleGenerateText} className="space-y-4 mt-4">
                                <div>
                                    <label htmlFor="promptInput" className="block text-sm font-medium text-gray-600">
                                        Промпт (необязательно):
                                    </label>
                                    <input
                                        id="promptInput"
                                        type="text"
                                        value={promptInput}
                                        onChange={(e) => setPromptInput(e.target.value)}
                                        placeholder="Начать с этих слов..."
                                        className="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm text-black"
                                    />
                                </div>
                                <div>
                                    <label htmlFor="maxLengthInput" className="block text-sm font-medium text-gray-600">
                                        Макс. длина (слова):
                                    </label>
                                    <input
                                        id="maxLengthInput"
                                        type="number"
                                        value={maxLengthInput}
                                        onChange={(e) => setMaxLengthInput(parseInt(e.target.value, 10) || 0)}
                                        min="10"
                                        max="500"
                                        className="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm text-black"
                                    />
                                </div>
                                <button
                                    type="submit"
                                    className="w-full px-4 py-2 text-sm font-medium text-white bg-teal-600 rounded-md hover:bg-teal-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-teal-500 transition-colors duration-150"
                                >
                                    Сгенерировать Текст
                                </button>
                            </form>
                        </section>
                    </div>

                    <section className="w-full p-6 bg-white rounded-lg shadow-xl md:col-span-1">
                        <h2 className="text-2xl font-semibold mb-4 text-gray-700 border-b pb-2">
                            3. Семантический Поиск
                        </h2>
                        <form onSubmit={handleSemanticSearch} className="space-y-4 mt-4">
                            <div>
                                <label htmlFor="searchQueryInput" className="block text-sm font-medium text-gray-600">
                                    Поисковый запрос:
                                </label>
                                <textarea
                                    id="searchQueryInput"
                                    rows={3}
                                    value={searchQueryInput}
                                    onChange={(e) => setSearchQueryInput(e.target.value)}
                                    placeholder="Введите текст для поиска..."
                                    className="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-green-500 focus:border-green-500 sm:text-sm text-black"
                                />
                            </div>
                            <div>
                                <label htmlFor="searchTopKInput" className="block text-sm font-medium text-gray-600">
                                    Количество результатов (top_k):
                                </label>
                                <input
                                    id="searchTopKInput"
                                    type="number"
                                    value={searchTopKInput}
                                    onChange={(e) => setSearchTopKInput(parseInt(e.target.value, 10) || 1)}
                                    min="1"
                                    max="20"
                                    className="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-green-500 focus:border-green-500 sm:text-sm text-black"
                                />
                            </div>
                            <button
                                type="submit"
                                className="w-full px-4 py-2 text-sm font-medium text-white bg-green-600 rounded-md hover:bg-green-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-500 transition-colors duration-150"
                            >
                                Найти семантически
                            </button>
                        </form>

                        {searchStatusMessage && (
                            <p
                                className={`mt-4 p-3 text-sm rounded-md ${searchResults.length > 0 && !searchStatusMessage.toLowerCase().includes('ошибка') ? 'bg-green-100 border border-green-300 text-green-700' : 'bg-blue-100 border border-blue-300 text-blue-700'}`}
                            >
                                {searchStatusMessage}
                            </p>
                        )}

                        {searchResults.length > 0 && (
                            <div className="mt-6">
                                <h3 className="text-xl font-medium mb-3 text-gray-600">Результаты поиска:</h3>
                                <ul className="space-y-4 max-h-[60vh] overflow-y-auto pr-2">
                                    {searchResults.map((item) => (
                                        <li
                                            key={item.qdrant_point_id}
                                            className="p-4 bg-gray-50 rounded-md border border-gray-200 shadow-sm"
                                        >
                                            <p className="text-sm text-gray-700 break-words">
                                                <strong>Текст:</strong> {item.payload.sentence_text}
                                            </p>
                                            <p className="text-xs text-gray-500 mt-1">
                                                <strong>Источник:</strong>{' '}
                                                <a
                                                    href={item.payload.source_url}
                                                    target="_blank"
                                                    rel="noopener noreferrer"
                                                    className="text-indigo-600 hover:underline"
                                                >
                                                    {item.payload.source_url}
                                                </a>
                                            </p>
                                            <p className="text-xs text-gray-500">
                                                <strong>Схожесть:</strong> {item.score.toFixed(4)}
                                            </p>
                                            <p className="text-xs text-gray-500">
                                                <strong>ID Документа:</strong> {item.payload.original_document_id}
                                            </p>
                                            <p className="text-xs text-gray-500">
                                                <strong>ID в Qdrant:</strong> {item.qdrant_point_id}
                                            </p>
                                        </li>
                                    ))}
                                </ul>
                            </div>
                        )}
                    </section>
                </div>

                <section className="w-full p-6 bg-white rounded-lg shadow-xl mt-8">
                    <h2 className="text-2xl font-semibold mb-4 text-gray-700 border-b pb-2">
                        Статус / Результаты Генерации
                    </h2>
                    {statusMessage && (
                        <p className="mb-4 p-3 text-sm text-gray-700 bg-blue-100 border border-blue-300 rounded-md">
                            {statusMessage}
                        </p>
                    )}
                    <h3 className="text-xl font-medium mb-2 text-gray-600">Сгенерированный текст:</h3>
                    <pre className="p-4 bg-gray-100 rounded-md whitespace-pre-wrap break-words text-sm text-gray-700 min-h-[100px]">
                        {generatedText}
                    </pre>
                </section>
            </div>
        </main>
    );
}
