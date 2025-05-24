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

export default function HomePage() {
    const [urlInput, setUrlInput] = useState<string>('');
    const [promptInput, setPromptInput] = useState<string>('');
    const [maxLengthInput, setMaxLengthInput] = useState<number>(50);
    const [statusMessage, setStatusMessage] = useState<string>('');
    const [generatedText, setGeneratedText] = useState<string>('... тут будет сгенерированный текст ...');
    const [sseEventSource, setSseEventSource] = useState<EventSource | null>(null);

    const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8080/api';

    useEffect(() => {
        const evtSource = new EventSource(`${API_BASE_URL}/events`);
        setSseEventSource(evtSource);
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
            evtSource.close(); // Закрываем при ошибке
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

    return (
        <main className="flex min-h-screen flex-col items-center justify-between p-12 md:p-24 bg-gray-50 text-gray-800">
            <div className="z-10 w-full max-w-5xl items-center justify-between font-mono text-sm lg:flex flex-col space-y-8">
                <h1 className="text-4xl font-bold text-center text-indigo-700">Codename: Symbiont UI</h1>

                <section className="w-full p-6 bg-white rounded-lg shadow-md">
                    <h2 className="text-2xl font-semibold mb-4 text-gray-700">1. Отправить URL на обработку</h2>
                    <form onSubmit={handleSubmitUrl} className="space-y-4">
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
                            className="w-full px-4 py-2 text-sm font-medium text-white bg-indigo-600 rounded-md hover:bg-indigo-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500"
                        >
                            Отправить URL
                        </button>
                    </form>
                </section>

                <section className="w-full p-6 bg-white rounded-lg shadow-md">
                    <h2 className="text-2xl font-semibold mb-4 text-gray-700">2. Сгенерировать текст</h2>
                    <form onSubmit={handleGenerateText} className="space-y-4">
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
                            className="w-full px-4 py-2 text-sm font-medium text-white bg-teal-600 rounded-md hover:bg-teal-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-teal-500"
                        >
                            Сгенерировать Текст
                        </button>
                    </form>
                </section>

                <section className="w-full p-6 bg-white rounded-lg shadow-md">
                    <h2 className="text-2xl font-semibold mb-4 text-gray-700">Статус / Результаты</h2>
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
